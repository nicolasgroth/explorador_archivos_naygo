# Naygo — Fase atajos configurables: keymap personalizable (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-08. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Fase pedida por Nicolás durante el brainstorm de sizing: hacer que TODOS los atajos de
teclado sean **personalizables**. Va ANTES de sizing, para que F3 (y los demás) nazcan en
el mapa configurable en vez de hardcodearse y refactorizarse después.

Hoy el input está hardcodeado en dos lugares: `ui::input::map_key` (teclas sin modificador:
↑↓←/Enter/Backspace/Tab/Esc) y un cascade de `if ctrl && key_pressed(...)` en
`app.rs::handle_input` (Ctrl+C/X/V, Ctrl+N, Ctrl+Shift+N, Del, Shift+Del, F2, F5, F6, y
Alt+←/→). La sección "Atajos" de Configuración existe pero es SOLO-LECTURA (clave
`settings.shortcuts.readonly`: "la personalización llega después"). Esta fase la hace real.

### Decisiones tomadas en el brainstorm

1. **Modelo:** una acción puede tener VARIOS atajos (GoUp = [Backspace, ←]); un atajo
   concreto pertenece a UNA sola acción (eso define el conflicto). Keymap = acción → lista
   de combinaciones.
2. **Una combinación (`Chord`)** = una tecla (`KeyCode`) + flags `ctrl`/`shift`/`alt`.
3. **Todas las acciones son rebindeables** (incluidas ↑↓/Enter/Esc), con "Restaurar valores
   por defecto" (global y por-fila) como red de seguridad.
4. **Conflicto al reasignar:** se REASIGNA — el atajo pasa a la acción nueva y se quita de
   la que lo tenía, con un aviso claro (estilo VS Code). Un atajo nunca dispara dos cosas.
5. **Editor:** tabla inline con chips (estilo A): fila por acción, chips con × para quitar,
   "+ Agregar" captura inline ("presioná la combinación… Esc cancela"), ↺ por fila +
   "Restaurar todo" + buscador. Conflicto = banner que explica la reasignación.
6. **Persistencia:** `keybindings.json` propio (no dentro de settings.json), tolerante
   (ausente/corrupto → defaults; acción sin entrada → su default; acción desconocida →
   ignorada).
7. **Lookup en runtime:** `action_for(chord)` reemplaza `map_key` + el cascade hardcodeado.
8. **Mouse Back/Forward** quedan FIJOS (no son teclas; rebindeo de mouse fuera de alcance).
9. **Defaults idénticos a hoy** + se agrega `Action::ComputeSize` (para F3 de sizing, que
   nace en este mapa).

### Qué entra

- `core::keymap`: `KeyCode`, `Chord`, `KeyMap` (defaults/action_for/chords_for/bind/unbind/
  reset_action/reset_all) + serde tolerante. `Action` se MUEVE a core.
- `core::config`: `load_keymap`/`save_keymap` (reusan read_json/write_json).
- `ui`: `handle_input` refactorizado a `keymap.action_for(chord)`; editor en
  `settings_window/shortcuts.rs` (reemplaza la grid solo-lectura); captura que suspende el
  input global; `NaygoApp.keymap`.
- i18n: nombre legible por acción + textos del editor.

### Qué NO entra

- Rebindeo de botones del mouse (fijos).
- Perfiles/esquemas de atajos múltiples (un solo keymap).
- Secuencias de teclas (chords de 2 pasos tipo Emacs) — solo combinaciones simples.
- sizing (fase siguiente; solo se agrega la variante `Action::ComputeSize` ahora).
- Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

### Capa `core::keymap` (módulo nuevo)

```rust
/// Tecla lógica (espejo serializable de las teclas que la app usa). Sin egui.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyCode {
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    Enter, Backspace, Tab, Escape, Delete,
    F2, F3, F5, F6,
    Char(char), // letras/dígitos: 'c','x','v','n', etc. (normalizadas a minúscula)
}

/// Una combinación de teclas: tecla + modificadores.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chord {
    pub key: KeyCode,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    /// Combinación sin modificadores (atajo simple).
    pub fn plain(key: KeyCode) -> Chord { Chord { key, ctrl: false, shift: false, alt: false } }
}

/// Acción de alto nivel (MOVIDA desde ui::input). Enum puro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Action {
    MoveUp, MoveDown, Activate, Open, OpenWith, GoUp, GoBack, GoForward,
    SwitchPane, CancelListing, Copy, Cut, Paste, Delete, DeletePermanent,
    Rename, NewFile, NewDir, CopyToOther, MoveToOther, ComputeSize,
}

impl Action {
    /// Todas las acciones rebindeables, en orden de presentación para el editor.
    pub fn all() -> &'static [Action];
    /// Clave i18n del nombre legible de esta acción (p. ej. "action.copy").
    pub fn i18n_key(self) -> &'static str;
}

/// Mapa configurable: por cada acción, sus combinaciones.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyMap {
    bindings: Vec<(Action, Vec<Chord>)>, // orden estable = Action::all()
}

impl KeyMap {
    /// Atajos por defecto (estilo Windows/Commander) — IDÉNTICOS a los de hoy.
    pub fn defaults() -> KeyMap;
    /// Qué acción dispara este chord, si alguna. Reemplaza map_key + el cascade.
    pub fn action_for(&self, chord: &Chord) -> Option<Action>;
    /// Los chords asignados a una acción (para el editor).
    pub fn chords_for(&self, action: Action) -> &[Chord];
    /// Asigna `chord` a `action`. Si el chord ya era de otra acción, se lo quita y
    /// devuelve esa otra acción (conflicto reasignado). Si ya era de `action`, no-op.
    pub fn bind(&mut self, action: Action, chord: Chord) -> Option<Action>;
    /// Quita `chord` de `action`.
    pub fn unbind(&mut self, action: Action, chord: &Chord);
    /// Restaura los atajos por defecto de UNA acción.
    pub fn reset_action(&mut self, action: Action);
    /// Restaura TODO el mapa a defaults.
    pub fn reset_all(&mut self);
}
```
- **`action_for`**: recorre los bindings; como `bind` garantiza que un chord pertenece a una
  sola acción, el primer match es único. O(n) sobre ~20 acciones × pocos chords = trivial.
- **`Action` movido a core**: hoy en `ui::input`. Se mueve a `core::keymap`; `ui::input`
  lo re-exporta (`pub use naygo_core::keymap::Action;`) para minimizar el churn en los 7
  archivos de ui que lo usan (app, column_menu, docking, file_panel, tree_panel, toolbar,
  input). Se añade la variante `ComputeSize`.
- **Serde tolerante**: el `KeyMap` se (de)serializa como una lista `[{action, chords}]`. La
  deserialización MERGEA con defaults: empieza de `defaults()`, y por cada entrada del json
  con una acción CONOCIDA, reemplaza sus chords; acciones desconocidas se ignoran; acciones
  sin entrada conservan su default. Así un keybindings.json viejo (sin `ComputeSize`)
  obtiene el default de las acciones nuevas. Esto se hace en una fn `KeyMap::from_stored(raw)`
  o un `Deserialize` manual; el modelo interno `Vec<(Action,Vec<Chord>)>` no se deserializa
  directo.

### Capa `core::config` (persistencia)

- `pub fn load_keymap(dir: &Path) -> KeyMap` — `read_json` de `dir/keybindings.json` → si
  `Some(raw)`, `KeyMap::from_stored(raw)` (merge con defaults); si `None` (ausente/corrupto)
  → `KeyMap::defaults()`. Nunca crash.
- `pub fn save_keymap(dir: &Path, km: &KeyMap)` — `write_json` de la forma serializable del
  keymap a `dir/keybindings.json`.
- Reusa los helpers privados `read_json`/`write_json` ya existentes.

### Capa `ui`

- **`NaygoApp.keymap: KeyMap`** — cargado en `new` con `load_keymap(config_dir)`.
- **`handle_input` refactorizado**: en vez de `map_key` + el cascade de `if`, arma un
  `Chord` de lo presionado (mapeo `egui::Key`→`KeyCode` + `modifiers`→flags; las letras a
  `KeyCode::Char(lower)`), y llama `self.keymap.action_for(&chord)`; si `Some(action)`,
  `apply_action(action)`. Itera las teclas presionadas este frame (egui da los key_pressed).
  Se conserva: el typeahead (escribir para saltar — NO es atajo; sigue por
  `egui::Event::Text`), Alt+←/→ pasa a ser bindings normales (GoBack/GoForward con alt), y
  los botones de mouse Back/Forward (fijos, fuera del keymap). El `map_key`/`map_mouse_extra`
  viejos se eliminan o quedan solo para el mouse.
- **Suspensión durante captura**: igual que con `pending_dialog`/`pending_resume`, mientras
  el editor está capturando (`capturing.is_some()`) `handle_input` NO procesa atajos (así
  capturar "Supr" no borra archivos). Esc durante captura = cancelar la captura.
- **Editor** (`settings_window/shortcuts.rs`, reemplaza la grid solo-lectura): tabla inline.
  Por cada `Action::all()`: nombre (i18n) + chips de sus `chords_for`; cada chip con × →
  `unbind`; "+ Agregar" → entra en modo captura para esa acción; ↺ → `reset_action`. Arriba:
  buscador (filtra por nombre) + "Restaurar todo" → `reset_all`. El estado de captura vive en
  el editor/NaygoApp (`shortcut_capture: Option<Action>`). Al capturar un chord:
  `keymap.bind(action, chord)` → si devuelve `Some(otra)`, mostrar banner de conflicto
  ("{chord} estaba en {otra}; ahora dispara {action}"). Cada cambio → `save_keymap`.
- **Render de un Chord** a texto ("Ctrl+C", "F3", "↑"): un helper en ui (no en core; es
  presentación). Símbolos para flechas, "Ctrl/Shift/Alt+" para modificadores.

### Lo que NO cambia

`apply_action` (sigue recibiendo `Action` y ejecutando), el resto de la UI, los demás
módulos. Solo cambia DE DÓNDE sale la `Action` (del keymap, no del cascade) y dónde vive el
enum (core).

---

## 3. Flujo de datos

**Arranque:** `load_keymap(config_dir)` → `KeyMap` (defaults+merge) → `NaygoApp.keymap`.

**Runtime:** tecla → `handle_input` arma `Chord` → `keymap.action_for(&chord)` → `Some`?
→ `apply_action`. (typeahead y mouse aparte.)

**Editar:** "+ Agregar"/captura → `shortcut_capture = Some(action)`, input global suspendido
→ usuario presiona → `Chord` → `keymap.bind(action, chord)` → conflicto? banner → `save_keymap`
→ captura off. ×/↺/Restaurar todo → unbind/reset_action/reset_all → save_keymap.

## 4. Manejo de errores / casos límite

- **keybindings.json ausente/corrupto** → `defaults()`. Sin crash.
- **Acción sin entrada en el json** (p. ej. `ComputeSize` en un json viejo) → su default.
- **Acción desconocida en el json** (de una versión futura) → ignorada.
- **Captura suspende el input** → "Supr"/"C" durante captura no disparan acciones.
- **Solo un modificador presionado** (Ctrl sin tecla) → no se captura; se espera una tecla
  real.
- **Tecla fuera de `KeyCode`** → el editor la ignora (no se puede asignar lo que el runtime
  no sabe leer).
- **Acción sin atajos** (quitar todos los chips) → permitido; inaccesible por teclado hasta
  reasignar; reset la recupera. (Menú/toolbar donde aplique siguen.)
- **Conflicto** → reasignación: el chord pasa a la acción nueva, se quita de la vieja.
- **Default sin regresiones** → `defaults()` reproduce EXACTAMENTE los atajos actuales.

## 5. Testing

- **`core::keymap`** (el grueso puro): `defaults()` tiene los atajos esperados (Ctrl+C→Copy,
  F2→Rename, Backspace y ← ambos→GoUp, Tab→SwitchPane, Esc→CancelListing, Ctrl+Shift+N→NewDir
  vs Ctrl+N→NewFile, Shift+Del→DeletePermanent vs Del→Delete, Alt+←→GoBack, F5→CopyToOther,
  F6→MoveToOther, etc.); `action_for(chord)` resuelve el correcto y `None` para uno libre;
  `bind` asigna y, ante conflicto, devuelve la acción despojada y la deja sin ese chord;
  `bind` del mismo chord a la misma acción es no-op; `unbind` quita; `reset_action`/`reset_all`
  restauran; varios-atajos-por-acción funciona; un chord pertenece a una sola acción tras un
  bind conflictivo.
- **Serde del KeyMap**: round-trip; json sin una acción → esa acción toma default; json con
  acción desconocida → ignorada; json corrupto → defaults (vía load_keymap con tempfile).
- **`Action::all()`/`i18n_key()`**: cubre las 21 variantes; i18n_key único por acción.
- **Mapeo egui→Chord** (si separable como fn pura) — testear; si no, smoke manual.
- **UI editor**: validación manual (capturar, conflicto+banner, ×, ↺, restaurar todo,
  buscador, dejar sin atajo, persistir al reiniciar, suspensión durante captura).

Meta: build + tests + clippy + fmt verde antes de cada commit.

---

## 6. Estructura de archivos (incremental)

```
crates/core/src/
├── keymap.rs        # NUEVO: KeyCode, Chord, Action (movido), KeyMap (+serde tolerante)
├── config/mod.rs    # + load_keymap / save_keymap (reusan read_json/write_json)
├── lib.rs           # + pub mod keymap;
└── i18n/{es,en}.json # + action.* (nombre por acción) + settings.shortcuts.* (editor)

crates/ui/src/
├── input.rs         # re-exporta Action de core; quita map_key (o lo deja solo para mouse);
│                    #   helper de mapeo egui::Key→KeyCode + chord_text (render del chord)
├── app.rs           # NaygoApp.keymap + shortcut_capture; handle_input → action_for(chord);
│                    #   suspensión durante captura
├── settings_window/shortcuts.rs  # editor inline con chips (reemplaza la grid solo-lectura)
└── (los 7 archivos que importan Action: ajustar el use si hace falta tras mover a core)
```

---

## 7. Dependencias

Ninguna nueva. `serde`/`serde_json` (keymap). Sin chrono.

---

## Fuera de alcance (recordatorio)

Rebindeo de mouse, perfiles múltiples, secuencias de 2 teclas, sizing (solo se agrega la
variante `ComputeSize`). Nunca: reproducción de media, edición de archivos.
