# atajos configurables — Keymap personalizable — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que todos los atajos de teclado sean personalizables: un keymap serializable en `core`, persistido en `keybindings.json`, editable en Configuración (capturar combinación, reasignar en conflicto, resetear), con `handle_input` consultando el mapa en vez de teclas hardcodeadas.

**Architecture:** `core::keymap` define `KeyCode`/`Chord`/`Action`/`KeyMap` (puro, serde tolerante con merge-a-defaults); `core::config` persiste `keybindings.json`; `ui` arma un `Chord` de cada tecla y consulta `keymap.action_for(chord)`, y la sección Atajos pasa de solo-lectura a un editor inline con chips. `Action` se mueve de `ui::input` a `core::keymap` (re-exportado para minimizar churn).

**Tech Stack:** Rust, `naygo-core`/`naygo-ui`, `eframe`/`egui` 0.34.3, `serde`/`serde_json`. Sin chrono, sin dependencias nuevas.

**Estado de partida (rama `feat/atajos-configurables`, desde `main` con ops-A/B/paste/shell-A/watcher):**
- `crates/ui/src/input.rs`: `pub enum Key { ArrowUp, ArrowDown, ArrowLeft, Enter, Backspace, Tab, Escape }`; `pub enum Action { MoveUp, MoveDown, Activate, Open, OpenWith, GoUp, GoBack, GoForward, SwitchPane, CancelListing, Copy, Cut, Paste, Delete, DeletePermanent, Rename, NewFile, NewDir, CopyToOther, MoveToOther }` (20, `#[derive(Clone,Copy,Debug,PartialEq,Eq)]`); `pub fn map_key(Key) -> Option<Action>` (↑↓→MoveUp/Down, Enter→Activate, Backspace|ArrowLeft→GoUp, Tab→SwitchPane, Escape→CancelListing); `pub enum MouseExtra { Back, Forward }`; `pub fn map_mouse_extra(MouseExtra) -> Action`. Tests del mapeo.
- `Action` se importa en 7 archivos de ui: `app.rs` (`use crate::input::{map_key, map_mouse_extra, Action, Key as NaygoKey, MouseExtra}`), `column_menu.rs`, `docking.rs`, `panes/file_panel.rs`, `panes/tree_panel.rs`, `toolbar.rs`, `input.rs`.
- `app.rs::handle_input` (~1817-1915): construye un `keys` array de `(egui::Key, NaygoKey)` para el mapa simple, maneja `Alt+ArrowLeft/Right`→GoBack/GoForward aparte, y un cascade de `if`:
  - `ctrl && C/X/V` → Copy/Cut/Paste; `ctrl && shift && N`→NewDir else `ctrl && N`→NewFile; `shift && Delete`→DeletePermanent else `Delete`→Delete; `F2`→Rename; `F5`→CopyToOther; `F6`→MoveToOther; `PointerButton::Extra1/Extra2`→map_mouse_extra; `egui::Event::Text`→typeahead.
  - Al final: `if !actions.is_empty() { self.typeahead_buf.clear(); }` luego `for a in actions { self.apply_action(a); }` y `if !typed.is_empty() { self.typeahead(&typed); }`.
  - Hay una guarda al inicio: `if self.pending_dialog.is_some() || !self.pending_resume.is_empty() { return; }` (suspende input bajo modal). Aquí se AÑADE `|| self.shortcut_capture.is_some()`.
- `crates/ui/src/settings_window/shortcuts.rs`: `pub fn show(ui, app: &mut NaygoApp)` — hoy una `egui::Grid` SOLO-LECTURA con filas `(&str, &str)` hardcodeadas + la nota `settings.shortcuts.readonly`. Se reemplaza entero.
- `crates/core/src/config/mod.rs`: helpers privados `fn read_json<T>(&Path) -> Option<T>` y `fn write_json<T>(&Path, &T)` (tolerantes, loguean, no crashean). `load_settings`/`save_settings` los usan. `portable_dir()`. `CONFIG_VERSION=1` (NO cambia).
- `crates/core/src/lib.rs`: declara `pub mod cancel/clipboard/columns/config/disk/filter/format/fs_model/i18n/icon_kind/listing/ops/sort/theme/tree/workspace`.
- i18n: `crates/core/src/i18n/{es,en}.json` planos; parity test. Hoy existen `shortcut.move/activate/up/backforward/switchpane/cancel` (de la grid vieja) y `settings.shortcuts`/`settings.shortcuts.readonly`.
- `NaygoApp` (app.rs ~200): campos varios (config_dir, settings, i18n, listings, watchers, etc.). `new(cc)` construye el struct.

**Prerequisito:** Rust en PATH. PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA `2>&1` con cargo. `cargo fmt --all -- --check`. Bash: NO `cd /d`.

**Convenciones (CLAUDE.md):** inglés en código; comentarios/commits español OK. Header de 2 líneas en archivos NUEVOS. `core` NUNCA importa egui/windows. Tolerante (config hostil, sin panics). Build+tests+clippy `--workspace --all-targets -- -D warnings`+fmt antes de cada commit. SIEMPRE `cargo fmt --all` antes de commitear. Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/atajos-configurables`. NO cambiar de rama.

**SECUENCIA:** Task 1 (mover Action a core + tipos) es cross-crate y debe dejar todo compilando (el re-export `pub use` mantiene los 7 importadores). Tasks 2-3 (KeyMap ops + serde) core puro. Task 4 (config load/save) core. Task 5 (refactor handle_input) cambia el runtime del input — debe mantener el default idéntico. Task 6 (editor UI). Task 7 cierre.

**Alcance:** ENTRA: core::keymap (KeyCode/Chord/Action/KeyMap), load/save_keymap, refactor de handle_input, editor inline en shortcuts.rs, i18n. NO ENTRA: rebindeo de mouse (fijo), perfiles, secuencias de 2 teclas, sizing (solo se agrega la variante ComputeSize).

---

## Estructura de archivos

```
crates/core/src/
├── keymap.rs        # NUEVO: KeyCode, Chord, Action (movido), KeyMap (+serde tolerante con merge)
├── config/mod.rs    # + load_keymap / save_keymap
├── lib.rs           # + pub mod keymap;
└── i18n/{es,en}.json # + action.* (21) + settings.shortcuts.* (editor)

crates/ui/src/
├── input.rs         # re-exporta Action; conserva map_mouse_extra/MouseExtra; + egui→KeyCode + chord_text
├── app.rs           # NaygoApp.keymap + shortcut_capture; handle_input → action_for(chord); suspensión en captura
└── settings_window/shortcuts.rs  # editor inline con chips (reemplaza la grid solo-lectura)
```

---

## Task 1: `core::keymap` — KeyCode, Chord, Action (movido), esqueleto KeyMap

**Files:**
- Create: `crates/core/src/keymap.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/ui/src/input.rs`

- [ ] **Step 1: Crear keymap.rs con los tipos + Action + un test trivial**

Create `crates/core/src/keymap.rs`:
```rust
// Naygo — keymap configurable: combinaciones de teclas → acciones (puro, serde).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Modelo PURO de atajos de teclado: `KeyCode`/`Chord`/`Action`/`KeyMap`. Sin egui ni
//! Windows. `action_for` resuelve qué acción dispara una combinación (lo consume la UI);
//! el editor usa `chords_for`/`bind`/`unbind`/`reset_*`. Serde tolerante: un json viejo
//! o incompleto se mergea con los defaults.

use serde::{Deserialize, Serialize};

/// Tecla lógica (espejo serializable de las teclas que la app usa). Sin egui.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyCode {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Enter,
    Backspace,
    Tab,
    Escape,
    Delete,
    F2,
    F3,
    F5,
    F6,
    /// Letra o dígito; normalizada a minúscula al construirla.
    Char(char),
}

/// Una combinación: tecla + modificadores.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chord {
    pub key: KeyCode,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    /// Combinación sin modificadores.
    pub fn plain(key: KeyCode) -> Chord {
        Chord { key, ctrl: false, shift: false, alt: false }
    }
    /// Con Ctrl.
    pub fn ctrl(key: KeyCode) -> Chord {
        Chord { key, ctrl: true, shift: false, alt: false }
    }
    /// Con Ctrl+Shift.
    pub fn ctrl_shift(key: KeyCode) -> Chord {
        Chord { key, ctrl: true, shift: true, alt: false }
    }
    /// Con Shift.
    pub fn shift(key: KeyCode) -> Chord {
        Chord { key, ctrl: false, shift: true, alt: false }
    }
    /// Con Alt.
    pub fn alt(key: KeyCode) -> Chord {
        Chord { key, ctrl: false, shift: false, alt: true }
    }
}

/// Acción de alto nivel (movida desde `ui::input`). Enum puro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    MoveUp,
    MoveDown,
    Activate,
    Open,
    OpenWith,
    GoUp,
    GoBack,
    GoForward,
    SwitchPane,
    CancelListing,
    Copy,
    Cut,
    Paste,
    Delete,
    DeletePermanent,
    Rename,
    NewFile,
    NewDir,
    CopyToOther,
    MoveToOther,
    /// Calcular el tamaño de la carpeta enfocada/seleccionada (fase sizing).
    ComputeSize,
}

impl Action {
    /// Todas las acciones, en orden de presentación para el editor.
    pub fn all() -> &'static [Action] {
        use Action::*;
        &[
            MoveUp, MoveDown, Activate, Open, OpenWith, GoUp, GoBack, GoForward,
            SwitchPane, CancelListing, Copy, Cut, Paste, Delete, DeletePermanent,
            Rename, NewFile, NewDir, CopyToOther, MoveToOther, ComputeSize,
        ]
    }

    /// Clave i18n del nombre legible de la acción.
    pub fn i18n_key(self) -> &'static str {
        use Action::*;
        match self {
            MoveUp => "action.move_up",
            MoveDown => "action.move_down",
            Activate => "action.activate",
            Open => "action.open",
            OpenWith => "action.open_with",
            GoUp => "action.go_up",
            GoBack => "action.go_back",
            GoForward => "action.go_forward",
            SwitchPane => "action.switch_pane",
            CancelListing => "action.cancel_listing",
            Copy => "action.copy",
            Cut => "action.cut",
            Paste => "action.paste",
            Delete => "action.delete",
            DeletePermanent => "action.delete_permanent",
            Rename => "action.rename",
            NewFile => "action.new_file",
            NewDir => "action.new_dir",
            CopyToOther => "action.copy_to_other",
            MoveToOther => "action.move_to_other",
            ComputeSize => "action.compute_size",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tiene_21_acciones_con_clave_i18n_unica() {
        let all = Action::all();
        assert_eq!(all.len(), 21);
        let mut keys: Vec<&str> = all.iter().map(|a| a.i18n_key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 21, "cada acción tiene una clave i18n única");
    }

    #[test]
    fn chord_constructores() {
        assert_eq!(Chord::plain(KeyCode::F3), Chord { key: KeyCode::F3, ctrl: false, shift: false, alt: false });
        assert!(Chord::ctrl(KeyCode::Char('c')).ctrl);
        assert!(Chord::ctrl_shift(KeyCode::Char('n')).shift);
    }
}
```

- [ ] **Step 2: Declarar el módulo en core**

Modify `crates/core/src/lib.rs`: add `pub mod keymap;`.

- [ ] **Step 3: Re-exportar Action en ui::input y quitar su definición**

Modify `crates/ui/src/input.rs`:
- DELETE the `pub enum Action { ... }` definition.
- Add at the top (after the header/doc): `pub use naygo_core::keymap::Action;`
- Keep `Key`, `map_key`, `MouseExtra`, `map_mouse_extra` for now (Task 5 trims them).
- `map_key` returns `Option<Action>` — still works since `Action` is re-exported.
NOTE: the 7 ui files import `Action` via `crate::input::Action` or `use crate::input::...Action...`. The re-export keeps those paths valid — verify by building. If any file imports a now-removed item, fix its `use`.

- [ ] **Step 4: Verificar**

Run: `cargo test -p naygo-core keymap` → 2 tests PASS.
Run: `cargo build --workspace` → compiles (the re-export keeps the 7 importers working).
Run: `cargo clippy --workspace --all-targets -- -D warnings` → clean.

- [ ] **Step 5: Commit**
```
git add crates/core/src/keymap.rs crates/core/src/lib.rs crates/ui/src/input.rs
git commit -m "feat(core): keymap — KeyCode/Chord/Action (movido de ui)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `KeyCode`/`Chord`/`Action` (21 variants)/`Action::all()`/`i18n_key()` EXACTOS (Tasks 2-6 depend).

---

## Task 2: `KeyMap` — defaults + action_for + chords_for

**Files:**
- Modify: `crates/core/src/keymap.rs`

- [ ] **Step 1: Tests (TDD)**

Añadir al `mod tests`:
```rust
    #[test]
    fn defaults_atajos_clave() {
        let km = KeyMap::defaults();
        assert_eq!(km.action_for(&Chord::ctrl(KeyCode::Char('c'))), Some(Action::Copy));
        assert_eq!(km.action_for(&Chord::ctrl(KeyCode::Char('x'))), Some(Action::Cut));
        assert_eq!(km.action_for(&Chord::ctrl(KeyCode::Char('v'))), Some(Action::Paste));
        assert_eq!(km.action_for(&Chord::plain(KeyCode::F2)), Some(Action::Rename));
        assert_eq!(km.action_for(&Chord::plain(KeyCode::F3)), Some(Action::ComputeSize));
        assert_eq!(km.action_for(&Chord::plain(KeyCode::F5)), Some(Action::CopyToOther));
        assert_eq!(km.action_for(&Chord::plain(KeyCode::F6)), Some(Action::MoveToOther));
        assert_eq!(km.action_for(&Chord::ctrl_shift(KeyCode::Char('n'))), Some(Action::NewDir));
        assert_eq!(km.action_for(&Chord::ctrl(KeyCode::Char('n'))), Some(Action::NewFile));
        assert_eq!(km.action_for(&Chord::shift(KeyCode::Delete)), Some(Action::DeletePermanent));
        assert_eq!(km.action_for(&Chord::plain(KeyCode::Delete)), Some(Action::Delete));
        assert_eq!(km.action_for(&Chord::plain(KeyCode::Enter)), Some(Action::Activate));
        assert_eq!(km.action_for(&Chord::plain(KeyCode::Tab)), Some(Action::SwitchPane));
        assert_eq!(km.action_for(&Chord::plain(KeyCode::Escape)), Some(Action::CancelListing));
        assert_eq!(km.action_for(&Chord::alt(KeyCode::ArrowLeft)), Some(Action::GoBack));
        assert_eq!(km.action_for(&Chord::alt(KeyCode::ArrowRight)), Some(Action::GoForward));
    }

    #[test]
    fn go_up_tiene_dos_atajos() {
        let km = KeyMap::defaults();
        assert_eq!(km.action_for(&Chord::plain(KeyCode::Backspace)), Some(Action::GoUp));
        assert_eq!(km.action_for(&Chord::plain(KeyCode::ArrowLeft)), Some(Action::GoUp));
        let chords = km.chords_for(Action::GoUp);
        assert_eq!(chords.len(), 2);
    }

    #[test]
    fn action_for_libre_es_none() {
        let km = KeyMap::defaults();
        // Ctrl+Z no está asignado por defecto.
        assert_eq!(km.action_for(&Chord::ctrl(KeyCode::Char('z'))), None);
    }
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core keymap` → ERROR: `KeyMap` no existe.

- [ ] **Step 3: Implementar KeyMap + defaults + action_for + chords_for**

Añadir a keymap.rs:
```rust
/// Mapa configurable: por cada acción, sus combinaciones. Orden estable = `Action::all()`.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyMap {
    bindings: Vec<(Action, Vec<Chord>)>,
}

impl KeyMap {
    /// Atajos por defecto (estilo Windows/Commander) — idénticos a los de la versión previa.
    pub fn defaults() -> KeyMap {
        use Action::*;
        use KeyCode::*;
        let b: Vec<(Action, Vec<Chord>)> = vec![
            (MoveUp, vec![Chord::plain(ArrowUp)]),
            (MoveDown, vec![Chord::plain(ArrowDown)]),
            (Activate, vec![Chord::plain(Enter)]),
            (Open, vec![]),     // sin atajo de teclado por defecto (vive en el menú contextual)
            (OpenWith, vec![]), // idem
            (GoUp, vec![Chord::plain(Backspace), Chord::plain(ArrowLeft)]),
            (GoBack, vec![Chord::alt(ArrowLeft)]),
            (GoForward, vec![Chord::alt(ArrowRight)]),
            (SwitchPane, vec![Chord::plain(Tab)]),
            (CancelListing, vec![Chord::plain(Escape)]),
            (Copy, vec![Chord::ctrl(Char('c'))]),
            (Cut, vec![Chord::ctrl(Char('x'))]),
            (Paste, vec![Chord::ctrl(Char('v'))]),
            (Delete, vec![Chord::plain(KeyCode::Delete)]),
            (DeletePermanent, vec![Chord::shift(KeyCode::Delete)]),
            (Rename, vec![Chord::plain(F2)]),
            (NewFile, vec![Chord::ctrl(Char('n'))]),
            (NewDir, vec![Chord::ctrl_shift(Char('n'))]),
            (CopyToOther, vec![Chord::plain(F5)]),
            (MoveToOther, vec![Chord::plain(F6)]),
            (ComputeSize, vec![Chord::plain(F3)]),
        ];
        KeyMap { bindings: b }
    }

    /// Qué acción dispara este chord, si alguna. Un chord pertenece a una sola acción.
    pub fn action_for(&self, chord: &Chord) -> Option<Action> {
        self.bindings
            .iter()
            .find(|(_, chords)| chords.contains(chord))
            .map(|(a, _)| *a)
    }

    /// Los chords asignados a una acción (vacío si no tiene).
    pub fn chords_for(&self, action: Action) -> &[Chord] {
        self.bindings
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, c)| c.as_slice())
            .unwrap_or(&[])
    }

    /// Acceso interno a la lista de chords mutable de una acción (la crea si falta).
    fn slot_mut(&mut self, action: Action) -> &mut Vec<Chord> {
        if let Some(pos) = self.bindings.iter().position(|(a, _)| *a == action) {
            &mut self.bindings[pos].1
        } else {
            self.bindings.push((action, Vec::new()));
            &mut self.bindings.last_mut().unwrap().1
        }
    }
}
```
NOTE: `defaults()` reproduce EXACTAMENTE los atajos del cascade actual de `handle_input` (verificar uno a uno contra app.rs ~1863). `Open`/`OpenWith` no tenían atajo de teclado (se disparan desde el menú contextual), así que arrancan sin chords — el usuario puede asignarles uno. `ComputeSize` (F3) es nueva, para sizing.

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core keymap` → todos PASS.
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 5: Commit**
```
git add crates/core/src/keymap.rs
git commit -m "feat(core): KeyMap defaults + action_for + chords_for

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `KeyMap`/`defaults`/`action_for`/`chords_for` EXACTOS (Tasks 4-6 depend).

---

## Task 3: `KeyMap` — bind / unbind / reset + serde tolerante

**Files:**
- Modify: `crates/core/src/keymap.rs`

- [ ] **Step 1: Tests (TDD)**

Añadir al `mod tests`:
```rust
    #[test]
    fn bind_conflicto_reasigna_y_devuelve_la_despojada() {
        let mut km = KeyMap::defaults();
        // Asignar Ctrl+C (de Copy) a Rename → debe quitárselo a Copy y devolver Copy.
        let robbed = km.bind(Action::Rename, Chord::ctrl(KeyCode::Char('c')));
        assert_eq!(robbed, Some(Action::Copy));
        assert_eq!(km.action_for(&Chord::ctrl(KeyCode::Char('c'))), Some(Action::Rename));
        assert!(!km.chords_for(Action::Copy).contains(&Chord::ctrl(KeyCode::Char('c'))));
    }

    #[test]
    fn bind_mismo_chord_misma_accion_es_noop() {
        let mut km = KeyMap::defaults();
        let r = km.bind(Action::Copy, Chord::ctrl(KeyCode::Char('c')));
        assert_eq!(r, None);
        assert_eq!(km.chords_for(Action::Copy).len(), 1);
    }

    #[test]
    fn bind_nuevo_libre_no_conflicto() {
        let mut km = KeyMap::defaults();
        let r = km.bind(Action::Copy, Chord::ctrl(KeyCode::Char('z'))); // libre
        assert_eq!(r, None);
        assert_eq!(km.chords_for(Action::Copy).len(), 2);
    }

    #[test]
    fn unbind_quita() {
        let mut km = KeyMap::defaults();
        km.unbind(Action::GoUp, &Chord::plain(KeyCode::ArrowLeft));
        assert_eq!(km.chords_for(Action::GoUp).len(), 1);
        assert_eq!(km.action_for(&Chord::plain(KeyCode::ArrowLeft)), None);
    }

    #[test]
    fn reset_action_y_reset_all() {
        let mut km = KeyMap::defaults();
        km.unbind(Action::Copy, &Chord::ctrl(KeyCode::Char('c')));
        assert!(km.chords_for(Action::Copy).is_empty());
        km.reset_action(Action::Copy);
        assert_eq!(km.action_for(&Chord::ctrl(KeyCode::Char('c'))), Some(Action::Copy));

        km.unbind(Action::Rename, &Chord::plain(KeyCode::F2));
        km.reset_all();
        assert_eq!(km, KeyMap::defaults());
    }

    #[test]
    fn serde_round_trip() {
        let km = KeyMap::defaults();
        let json = serde_json::to_string(&km).unwrap();
        let back: KeyMap = serde_json::from_str(&json).unwrap();
        assert_eq!(back, km);
    }

    #[test]
    fn serde_merge_accion_faltante_toma_default() {
        // Un json que solo trae Copy (sin las demás) → las demás caen a su default.
        let json = r#"[{"action":"Copy","chords":[{"key":{"Char":"q"},"ctrl":true,"shift":false,"alt":false}]}]"#;
        let km: KeyMap = serde_json::from_str(json).unwrap();
        // Copy quedó con Ctrl+Q (del json).
        assert_eq!(km.action_for(&Chord::ctrl(KeyCode::Char('q'))), Some(Action::Copy));
        // Rename conserva su default F2 (no estaba en el json).
        assert_eq!(km.action_for(&Chord::plain(KeyCode::F2)), Some(Action::Rename));
    }
```
NOTE: el formato exacto del json de `Chord`/`KeyCode::Char` depende de cómo serde serialice el enum — el test `serde_merge_*` usa una forma plausible (`{"key":{"Char":"q"},...}`); AJUSTAR el string del test al formato real que produzca `serde_json::to_string` de un `Chord` (correr el round-trip primero, imprimir, y copiar el formato). La LÓGICA (merge con defaults) es lo fijo.

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core keymap` → ERROR: `bind`/`unbind`/`reset_action`/`reset_all` + Serialize/Deserialize de KeyMap no existen.

- [ ] **Step 3: Implementar bind/unbind/reset + serde**

Añadir a `impl KeyMap`:
```rust
    /// Asigna `chord` a `action`. Si el chord ya era de OTRA acción, se lo quita y devuelve
    /// esa otra acción (conflicto reasignado). Si ya era de `action`, no-op (None).
    pub fn bind(&mut self, action: Action, chord: Chord) -> Option<Action> {
        let owner = self.action_for(&chord);
        match owner {
            Some(a) if a == action => None, // ya lo tiene
            Some(a) => {
                // Quitárselo al dueño anterior.
                self.slot_mut(a).retain(|c| *c != chord);
                self.slot_mut(action).push(chord);
                Some(a)
            }
            None => {
                self.slot_mut(action).push(chord);
                None
            }
        }
    }

    /// Quita `chord` de `action`.
    pub fn unbind(&mut self, action: Action, chord: &Chord) {
        self.slot_mut(action).retain(|c| c != chord);
    }

    /// Restaura los atajos por defecto de UNA acción.
    pub fn reset_action(&mut self, action: Action) {
        let def = KeyMap::defaults();
        let default_chords = def.chords_for(action).to_vec();
        // Quitar esos chords de quien los tenga (para no duplicar), luego asignarlos.
        for c in &default_chords {
            if let Some(owner) = self.action_for(c) {
                if owner != action {
                    self.slot_mut(owner).retain(|x| x != c);
                }
            }
        }
        *self.slot_mut(action) = default_chords;
    }

    /// Restaura TODO el mapa a defaults.
    pub fn reset_all(&mut self) {
        *self = KeyMap::defaults();
    }
```
Para serde, definir una forma serializable explícita (lista de entradas) + la lógica de merge. Añadir:
```rust
/// Forma serializable del keymap: lista de (acción, chords). Lo que va al json.
#[derive(Serialize, Deserialize)]
struct StoredBinding {
    action: Action,
    chords: Vec<Chord>,
}

impl Serialize for KeyMap {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let stored: Vec<StoredBinding> = self
            .bindings
            .iter()
            .map(|(a, c)| StoredBinding { action: *a, chords: c.clone() })
            .collect();
        stored.serialize(s)
    }
}

impl<'de> Deserialize<'de> for KeyMap {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<KeyMap, D::Error> {
        let stored: Vec<StoredBinding> = Vec::deserialize(d)?;
        Ok(KeyMap::from_stored(stored))
    }
}

impl KeyMap {
    /// Construye un KeyMap mergeando lo almacenado con los defaults: arranca de defaults,
    /// y por cada entrada con una acción CONOCIDA reemplaza sus chords. Acciones
    /// desconocidas se ignoran; acciones sin entrada conservan su default.
    fn from_stored(stored: Vec<StoredBinding>) -> KeyMap {
        let mut km = KeyMap::defaults();
        for entry in stored {
            // `entry.action` es una Action válida (deserializó), así que es "conocida".
            if let Some(pos) = km.bindings.iter().position(|(a, _)| *a == entry.action) {
                km.bindings[pos].1 = entry.chords;
            }
            // Si no estuviera en bindings (no debería, defaults las tiene todas), se ignora.
        }
        km
    }
}
```
NOTE: una acción DESCONOCIDA en el json (de una versión futura) hace fallar el `Action::deserialize` de ESA entrada → con `Vec::deserialize` eso aborta todo. Para tolerarlo de verdad, deserializar a `Vec<serde_json::Value>` y parsear cada entrada con `serde_json::from_value`, ignorando las que fallen. SIMPLIFICACIÓN aceptable: como hoy NO hay acciones que vayan a desaparecer (solo se agregan), y `from_stored` ya cubre "acción faltante → default", el riesgo real es bajo. Implementar la versión robusta (parseo entrada-por-entrada tolerante) si no complica; si complica, dejar la simple y anotar que una acción desconocida futura caería a defaults global (vía load_keymap que captura el error). Decide y documenta.

- [ ] **Step 4: Correr — pasan (ajustar el string del test serde al formato real)**

Run: `cargo test -p naygo-core keymap` → todos PASS. (Si `serde_merge_accion_faltante_toma_default` falla por el formato del json, imprimir `serde_json::to_string(&KeyMap::defaults())` y ajustar el string del test al formato real de `Chord`.)
Run: `cargo test -p naygo-core` → green.
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 5: Commit**
```
git add crates/core/src/keymap.rs
git commit -m "feat(core): KeyMap bind/unbind/reset + serde tolerante (merge a defaults)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `bind`/`unbind`/`reset_action`/`reset_all` + el serde EXACTOS (Tasks 4-6 depend).

---

## Task 4: `core::config` — load_keymap / save_keymap

**Files:**
- Modify: `crates/core/src/config/mod.rs`

- [ ] **Step 1: Tests (TDD)**

Añadir al `mod tests` de config/mod.rs:
```rust
    #[test]
    fn keymap_ausente_es_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let km = load_keymap(dir.path());
        assert_eq!(km, naygo_core::keymap::KeyMap::defaults());
    }

    #[test]
    fn keymap_round_trip_en_disco() {
        use naygo_core::keymap::{Action, Chord, KeyCode};
        let dir = tempfile::tempdir().unwrap();
        let mut km = naygo_core::keymap::KeyMap::defaults();
        km.bind(Action::Copy, Chord::ctrl(KeyCode::Char('z')));
        save_keymap(dir.path(), &km);
        let back = load_keymap(dir.path());
        assert_eq!(back.action_for(&Chord::ctrl(KeyCode::Char('z'))), Some(Action::Copy));
    }

    #[test]
    fn keymap_corrupto_es_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keybindings.json"), b"{ no es json").unwrap();
        let km = load_keymap(dir.path());
        assert_eq!(km, naygo_core::keymap::KeyMap::defaults());
    }
```
NOTE: si los tests de config no referencian `naygo_core::` (son del propio crate), usar `crate::keymap::...` en su lugar. Ajustar al patrón del archivo.

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core config::tests::keymap` → ERROR: `load_keymap`/`save_keymap` no existen.

- [ ] **Step 3: Implementar (en config/mod.rs, junto a load_settings/save_settings)**
```rust
/// Carga el keymap desde `keybindings.json`; ausente/corrupto → defaults.
pub fn load_keymap(dir: &Path) -> crate::keymap::KeyMap {
    read_json::<crate::keymap::KeyMap>(&dir.join("keybindings.json"))
        .unwrap_or_else(crate::keymap::KeyMap::defaults)
}

/// Guarda el keymap.
pub fn save_keymap(dir: &Path, km: &crate::keymap::KeyMap) {
    write_json(&dir.join("keybindings.json"), km);
}
```
(`read_json` ya devuelve `None` y loguea si el json es ilegible — el `unwrap_or_else(defaults)` cubre ausente y corrupto.)

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core config` → PASS. `cargo test -p naygo-core` → green. `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 5: Commit**
```
git add crates/core/src/config/mod.rs
git commit -m "feat(core): load_keymap / save_keymap (keybindings.json tolerante)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `load_keymap(&Path) -> KeyMap` / `save_keymap(&Path, &KeyMap)` EXACTOS (Tasks 5-6 depend).

---

## Task 5: UI — refactor de handle_input al keymap (sin regresiones)

**Files:**
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/ui/src/input.rs`

- [ ] **Step 1: Helper egui::Key → KeyCode (input.rs)**

Modify `crates/ui/src/input.rs`: add a mapping from egui to the core `KeyCode`, and a chord-text renderer (used by the editor in Task 6). `Key`/`map_key` can be removed (the keymap replaces them); keep `MouseExtra`/`map_mouse_extra`. Add:
```rust
use naygo_core::keymap::{Chord, KeyCode};

/// Traduce una `egui::Key` a nuestro `KeyCode`, si la soportamos.
pub fn egui_key_to_code(key: egui::Key) -> Option<KeyCode> {
    Some(match key {
        egui::Key::ArrowUp => KeyCode::ArrowUp,
        egui::Key::ArrowDown => KeyCode::ArrowDown,
        egui::Key::ArrowLeft => KeyCode::ArrowLeft,
        egui::Key::ArrowRight => KeyCode::ArrowRight,
        egui::Key::Enter => KeyCode::Enter,
        egui::Key::Backspace => KeyCode::Backspace,
        egui::Key::Tab => KeyCode::Tab,
        egui::Key::Escape => KeyCode::Escape,
        egui::Key::Delete => KeyCode::Delete,
        egui::Key::F2 => KeyCode::F2,
        egui::Key::F3 => KeyCode::F3,
        egui::Key::F5 => KeyCode::F5,
        egui::Key::F6 => KeyCode::F6,
        // Letras A..Z → Char(minúscula). egui::Key::A..=Z son variantes contiguas.
        other => {
            let name = format!("{other:?}"); // p. ej. "C", "N"
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphabetic() => {
                    KeyCode::Char(c.to_ascii_lowercase())
                }
                _ => return None,
            }
        }
    })
}

/// Texto legible de un chord para mostrar en el editor ("Ctrl+C", "F3", "↑").
pub fn chord_text(chord: &Chord) -> String {
    let mut s = String::new();
    if chord.ctrl { s.push_str("Ctrl+"); }
    if chord.shift { s.push_str("Shift+"); }
    if chord.alt { s.push_str("Alt+"); }
    let k = match chord.key {
        KeyCode::ArrowUp => "↑".to_string(),
        KeyCode::ArrowDown => "↓".to_string(),
        KeyCode::ArrowLeft => "←".to_string(),
        KeyCode::ArrowRight => "→".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Escape => "Esc".to_string(),
        KeyCode::Delete => "Supr".to_string(),
        KeyCode::F2 => "F2".to_string(),
        KeyCode::F3 => "F3".to_string(),
        KeyCode::F5 => "F5".to_string(),
        KeyCode::F6 => "F6".to_string(),
        KeyCode::Char(c) => c.to_ascii_uppercase().to_string(),
    };
    s.push_str(&k);
    s
}
```
NOTE: VERIFY `egui::Key` letter variants format as their letter via `Debug` ("C" for `egui::Key::C`) in egui 0.34.3 — if `format!("{:?}")` yields something else, map letters explicitly (egui::Key::A..egui::Key::Z) or use egui's `Key::name()`. The chord-from-input must be robust; prefer an explicit match over Debug-parsing if unsure. Adapt.

- [ ] **Step 2: NaygoApp.keymap + shortcut_capture (app.rs)**

Add fields to `struct NaygoApp`:
```rust
    /// Atajos de teclado configurables.
    keymap: naygo_core::keymap::KeyMap,
    /// Acción cuyo nuevo atajo se está capturando en el editor (suspende el input global).
    shortcut_capture: Option<naygo_core::keymap::Action>,
```
Init in `new`: `keymap: config::load_keymap(&config_dir),` and `shortcut_capture: None,`.

- [ ] **Step 3: Refactor handle_input**

Modify `crates/ui/src/app.rs::handle_input` (~1817):
a) The suspend guard at the top: add `|| self.shortcut_capture.is_some()`:
```rust
        if self.pending_dialog.is_some()
            || !self.pending_resume.is_empty()
            || self.shortcut_capture.is_some()
        {
            return;
        }
```
b) Replace the whole key-handling body (the `keys` array + `map_key` loop + the Alt+arrows + the entire cascade of `if ctrl && ...`) with: for each egui key pressed this frame, build a `Chord` and look it up. Keep the typeahead and the mouse-extra handling. Concretely:
```rust
        let mut actions = Vec::new();
        let mut typed = String::new();
        ctx.input(|i| {
            let (ctrl, shift, alt) = (i.modifiers.ctrl, i.modifiers.shift, i.modifiers.alt);
            // Recorrer las teclas presionadas este frame; armar un Chord y resolver.
            for ev in &i.events {
                if let egui::Event::Key { key, pressed: true, .. } = ev {
                    if let Some(code) = crate::input::egui_key_to_code(*key) {
                        let chord = naygo_core::keymap::Chord { key: code, ctrl, shift, alt };
                        if let Some(action) = self.keymap.action_for(&chord) {
                            actions.push(action);
                        }
                    }
                }
            }
            // Botones laterales del mouse (fijos, fuera del keymap).
            if i.pointer.button_pressed(egui::PointerButton::Extra1) {
                actions.push(crate::input::map_mouse_extra(crate::input::MouseExtra::Back));
            }
            if i.pointer.button_pressed(egui::PointerButton::Extra2) {
                actions.push(crate::input::map_mouse_extra(crate::input::MouseExtra::Forward));
            }
            // Typeahead: texto escrito que NO disparó un atajo.
            for event in &i.events {
                if let egui::Event::Text(t) = event {
                    typed.push_str(t);
                }
            }
        });
        if !actions.is_empty() {
            self.typeahead_buf.clear();
        }
        for a in actions {
            self.apply_action(a);
        }
        if !typed.is_empty() {
            self.typeahead(&typed);
        }
```
NOTE: `egui::Event::Key { key, pressed, modifiers, repeat }` is the 0.34 shape — VERIFY the field names (`pressed: true`, ignore `repeat` or skip repeats: add `repeat: false` if you don't want key-repeat to re-fire, but the old code used `key_pressed` which is edge-triggered; matching `pressed: true` includes repeats — to match old behavior, also require `!repeat` OR keep using `i.key_pressed(...)` per key. SIMPLER + closest to old behavior: iterate `Action`/chords differently — but the event-loop approach is cleanest. If repeats cause double-fire, filter `repeat == false`). Also: the `typed` (typeahead) must NOT include letters that were consumed as a Ctrl+letter shortcut — egui already doesn't emit `Text` for Ctrl+key combos (Ctrl+C doesn't produce a 'c' text event), so typeahead stays clean. Verify.
   IMPORTANT — keep behavior identical: the OLD code mapped Backspace/ArrowLeft→GoUp, Enter→Activate, etc. via defaults. Since `KeyMap::defaults()` reproduces all of them, the refactor preserves behavior. Test by running the app: arrows move, Enter activates, Ctrl+C copies, F2 renames, etc.
c) Remove the now-unused `map_key`/`Key as NaygoKey` import from app.rs's `use crate::input::{...}` (keep `map_mouse_extra, MouseExtra`). Remove `Key`/`map_key` from input.rs if nothing else uses them (grep).

- [ ] **Step 4: Build, lint, fmt, manual-ish**

Run: `cargo build --workspace` → compiles.
Run: `cargo test --workspace` → green (the input.rs map_key tests were removed with map_key; if they referenced removed items, delete them; keep map_mouse_extra tests).
Run: `cargo clippy --workspace --all-targets -- -D warnings` → clean.
Run: `cargo fmt --all` + `--check`.
MANUAL (report): run the app, confirm NO regression — ↑↓ move focus, Enter enters a folder, Backspace/← go up, Tab switches pane, Esc cancels, Ctrl+C/X/V, F2 rename, Del/Shift+Del, Ctrl+N/Ctrl+Shift+N, Alt+←/→, F5/F6, mouse back/forward all work as before. Typeahead (typing a letter to jump) still works.

- [ ] **Step 5: Commit**
```
git add crates/ui/src/app.rs crates/ui/src/input.rs
git commit -m "refactor(ui): handle_input usa el keymap (action_for) en vez de teclas hardcodeadas

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: UI — editor de atajos (reemplaza la grid solo-lectura) + i18n

**Files:**
- Modify: `crates/ui/src/settings_window/shortcuts.rs`
- Modify: `crates/ui/src/app.rs` (helpers de captura, si hace falta)
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: i18n (ambos archivos, idénticos)**

Add to es.json/en.json. Nombres de las 21 acciones (`action.*`) + textos del editor (`settings.shortcuts.*`). ES:
`"action.move_up": "Mover selección arriba"`, `"action.move_down": "Mover selección abajo"`, `"action.activate": "Entrar / abrir"`, `"action.open": "Abrir"`, `"action.open_with": "Abrir con…"`, `"action.go_up": "Subir un nivel"`, `"action.go_back": "Atrás"`, `"action.go_forward": "Adelante"`, `"action.switch_pane": "Cambiar de panel"`, `"action.cancel_listing": "Cancelar listado"`, `"action.copy": "Copiar"`, `"action.cut": "Cortar"`, `"action.paste": "Pegar"`, `"action.delete": "Eliminar (papelera)"`, `"action.delete_permanent": "Eliminar permanentemente"`, `"action.rename": "Renombrar"`, `"action.new_file": "Nuevo archivo"`, `"action.new_dir": "Nueva carpeta"`, `"action.copy_to_other": "Copiar al otro panel"`, `"action.move_to_other": "Mover al otro panel"`, `"action.compute_size": "Calcular tamaño"`, `"settings.shortcuts.add": "+ Agregar"`, `"settings.shortcuts.capturing": "Presioná la combinación… (Esc cancela)"`, `"settings.shortcuts.reset_all": "Restaurar todo"`, `"settings.shortcuts.reset_one": "Restaurar esta acción"`, `"settings.shortcuts.search": "Buscar acción…"`, `"settings.shortcuts.conflict": "{chord} estaba en «{from}»; ahora dispara «{to}»."`, `"settings.shortcuts.none": "(sin atajo)"`, `"settings.shortcuts.col_action": "Acción"`, `"settings.shortcuts.col_keys": "Atajo(s)"`.
EN equivalents (same keys): "Move selection up/down", "Enter / open", "Open", "Open with…", "Go up one level", "Back", "Forward", "Switch pane", "Cancel listing", "Copy", "Cut", "Paste", "Delete (Recycle Bin)", "Delete permanently", "Rename", "New file", "New folder", "Copy to other pane", "Move to other pane", "Compute size", "+ Add", "Press the combination… (Esc cancels)", "Restore all", "Restore this action", "Search action…", "{chord} was on \"{from}\"; now triggers \"{to}\".", "(no shortcut)", "Action", "Shortcut(s)".
(READ both json to match format; keep keys identical. The old `shortcut.*` keys from the read-only grid can be removed since the grid is replaced — but the i18n parity test requires removing them from BOTH files. Remove `shortcut.move/activate/up/backforward/switchpane/cancel` from both if no longer referenced; grep first.)

- [ ] **Step 2: Estado de captura + búsqueda en el editor**

The capture state lives in `NaygoApp.shortcut_capture` (added in Task 5). Add a search-string field if needed: `shortcut_search: String` in NaygoApp (init `String::new()`), OR keep it in egui memory inside the section. Simpler: a field on NaygoApp.

- [ ] **Step 3: Reescribir shortcuts.rs como editor inline**

Replace `crates/ui/src/settings_window/shortcuts.rs` `show` with:
```rust
// Naygo — sección Atajos: editor de keybindings configurable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use crate::input::chord_text;
use naygo_core::keymap::{Action, Chord};

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    ui.heading(app.tr("settings.shortcuts"));
    ui.add_space(6.0);

    // Barra: buscador + Restaurar todo.
    let search_label = app.tr("settings.shortcuts.search");
    let reset_all_label = app.tr("settings.shortcuts.reset_all");
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.shortcut_search).hint_text(search_label));
        if ui.button(reset_all_label).clicked() {
            app.keymap.reset_all();
            app.save_keymap_now();
        }
    });
    ui.add_space(4.0);

    // Aviso de conflicto (si lo hay) — se setea al reasignar.
    if let Some(msg) = app.shortcut_conflict.clone() {
        ui.label(egui::RichText::new(msg).color(egui::Color32::from_rgb(0xe0, 0xa0, 0x30)));
    }

    let query = app.shortcut_search.to_lowercase();
    // Recolectar acciones a mostrar (filtradas) + sus chords ANTES de mutar (borrow).
    let rows: Vec<(Action, String, Vec<Chord>)> = Action::all()
        .iter()
        .map(|a| (*a, app.tr(a.i18n_key()), app.keymap.chords_for(*a).to_vec()))
        .filter(|(_, name, _)| query.is_empty() || name.to_lowercase().contains(&query))
        .collect();

    let add_label = app.tr("settings.shortcuts.add");
    let capturing_label = app.tr("settings.shortcuts.capturing");
    let reset_one_label = app.tr("settings.shortcuts.reset_one");

    // Acciones diferidas (no mutar app.keymap mientras iteramos `rows`/pintamos).
    let mut to_unbind: Option<(Action, Chord)> = None;
    let mut to_capture: Option<Action> = None;
    let mut to_reset: Option<Action> = None;

    egui::Grid::new("shortcuts_editor").num_columns(3).striped(true).show(ui, |ui| {
        for (action, name, chords) in &rows {
            ui.label(name);
            ui.horizontal(|ui| {
                for c in chords {
                    let txt = chord_text(c);
                    ui.label(egui::RichText::new(&txt).monospace());
                    if ui.small_button("×").clicked() {
                        to_unbind = Some((*action, *c));
                    }
                }
                if app.shortcut_capture == Some(*action) {
                    ui.label(egui::RichText::new(&capturing_label).italics());
                } else if ui.button(&add_label).clicked() {
                    to_capture = Some(*action);
                }
            });
            if ui.button("↺").on_hover_text(&reset_one_label).clicked() {
                to_reset = Some(*action);
            }
            ui.end_row();
        }
    });

    // Aplicar las acciones diferidas + persistir.
    if let Some((a, c)) = to_unbind {
        app.keymap.unbind(a, &c);
        app.shortcut_conflict = None;
        app.save_keymap_now();
    }
    if let Some(a) = to_capture {
        app.shortcut_capture = Some(a);
        app.shortcut_conflict = None;
    }
    if let Some(a) = to_reset {
        app.keymap.reset_action(a);
        app.shortcut_conflict = None;
        app.save_keymap_now();
    }
}
```
NOTE: This references `app.shortcut_search: String`, `app.shortcut_conflict: Option<String>`, and `app.save_keymap_now()` — add these to NaygoApp (Step 4). `app.tr(key) -> String` exists. Adapt widget APIs to egui 0.34.3 (`TextEdit::singleline().hint_text`, `small_button`, `RichText::monospace/italics`). Match how other settings sections render.

- [ ] **Step 4: Captura del chord en NaygoApp + helpers**

In `NaygoApp`: add `shortcut_search: String` (init `String::new()`) and `shortcut_conflict: Option<String>` (init `None`). Add:
```rust
    /// Guarda el keymap a disco (tras cualquier edición).
    fn save_keymap_now(&mut self) {
        naygo_core::config::save_keymap(&self.config_dir, &self.keymap);
    }
```
The CAPTURE itself: while `shortcut_capture.is_some()`, `handle_input` returns early (Task 5), so the normal input is suspended. But we still need to READ the pressed combination to assign it. Add a dedicated capture step in the frame loop (e.g. at the START of the settings-window render, or in `update` before handle_input). Implement a method:
```rust
    /// Si estamos capturando un atajo, lee la próxima combinación presionada y la asigna.
    /// Esc cancela la captura. Debe llamarse cada frame (antes/independiente de handle_input).
    fn process_shortcut_capture(&mut self, ctx: &egui::Context) {
        let Some(action) = self.shortcut_capture else {
            return;
        };
        let mut captured: Option<naygo_core::keymap::Chord> = None;
        let mut cancel = false;
        ctx.input(|i| {
            let (ctrl, shift, alt) = (i.modifiers.ctrl, i.modifiers.shift, i.modifiers.alt);
            for ev in &i.events {
                if let egui::Event::Key { key, pressed: true, .. } = ev {
                    if *key == egui::Key::Escape && !ctrl && !shift && !alt {
                        cancel = true;
                        break;
                    }
                    if let Some(code) = crate::input::egui_key_to_code(*key) {
                        captured = Some(naygo_core::keymap::Chord { key: code, ctrl, shift, alt });
                        break;
                    }
                }
            }
        });
        if cancel {
            self.shortcut_capture = None;
            return;
        }
        if let Some(chord) = captured {
            let chord_txt = crate::input::chord_text(&chord);
            if let Some(robbed) = self.keymap.bind(action, chord) {
                // Conflicto: avisar de la reasignación.
                self.shortcut_conflict = Some(
                    self.i18n
                        .t("settings.shortcuts.conflict")
                        .replace("{chord}", &chord_txt)
                        .replace("{from}", &self.i18n.t(robbed.i18n_key()))
                        .replace("{to}", &self.i18n.t(action.i18n_key())),
                );
            } else {
                self.shortcut_conflict = None;
            }
            self.shortcut_capture = None;
            self.save_keymap_now();
        }
    }
```
Call `self.process_shortcut_capture(&ctx)` once per frame in `update`/`ui`, BEFORE `handle_input` (so a captured key isn't also processed as an action — and handle_input returns early anyway while capturing). NOTE: capturing only a modifier (no real key) → `captured` stays None → keeps waiting (correct). Esc with no modifiers cancels.

- [ ] **Step 5: Build, lint, fmt, manual**

Run: `cargo build --workspace`; `cargo test --workspace` → green (i18n parity); `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all` + `--check`.
MANUAL: open Settings → Atajos: the editor lists actions with chips; "+ Agregar" → "presioná la combinación", press Ctrl+J → it binds; assign an in-use chord (Ctrl+C) → conflict banner + it moves; × removes; ↺ resets one; "Restaurar todo" resets; search filters; persists across restart (check keybindings.json appears in the config dir).

- [ ] **Step 6: Commit**
```
git add crates/ui/src/settings_window/shortcuts.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): editor de atajos configurable (captura/conflicto/reset/buscar)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Cierre — README, verificación final, push

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README**

READ el bloque de estado y reemplazar con:
```markdown
> **Estado:** Fase atajos configurables (keymap personalizable) en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-08-naygo-atajos-configurables-design.md`](docs/superpowers/specs/2026-06-08-naygo-atajos-configurables-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-08-naygo-atajos-configurables.md`](docs/superpowers/plans/2026-06-08-naygo-atajos-configurables.md).
> Operaciones (ops-A/B), paste inteligente, shell-A, watcher y bloque visual completos.
```

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compiles.
Run: `cargo test --workspace` → green.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → clean.
Run: `cargo fmt --all -- --check` → clean.
Run: `cargo build --release -p naygo-ui` → release compiles.
MANUAL end-to-end: sin regresiones de atajos default; editor rebindea/reasigna/resetea; persiste.

- [ ] **Step 3: Commit y push**
```
git add README.md
git commit -m "chore: actualizar estado del README (fase atajos configurables)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/atajos-configurables
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| KeyCode/Chord/Action (movido) + all()/i18n_key() | 1 |
| Action::ComputeSize añadida | 1 |
| KeyMap defaults (idénticos a hoy) + action_for + chords_for | 2 |
| bind (reasigna conflicto) / unbind / reset_action / reset_all | 3 |
| serde tolerante con merge a defaults | 3 |
| load_keymap / save_keymap (keybindings.json) | 4 |
| handle_input → action_for(chord), sin regresiones | 5 |
| egui→KeyCode + chord_text | 5 |
| suspensión del input durante captura | 5 (guard) + 6 (capture) |
| editor inline con chips (×/+Agregar/↺/Restaurar todo/buscar) | 6 |
| conflicto → banner | 6 |
| i18n (21 acciones + editor) | 6 |
| mouse fijo / typeahead aparte | 5 |
| perfiles/secuencias/sizing FUERA | (no se tocan) |

**Notas de riesgo:**
- **Mover Action a core** (Task 1): el `pub use` re-export mantiene los 7 importadores; verificar que TODOS compilan; si alguno importaba `Action` con una ruta que cambia, ajustar.
- **handle_input sin regresiones** (Task 5): `defaults()` DEBE reproducir el cascade actual exactamente — verificar binding por binding contra app.rs ~1863. Cuidado con key-repeat (`egui::Event::Key { repeat }`): el código viejo era edge-triggered; si el nuevo doble-dispara por repeat, filtrar `repeat==false` o seguir usando `i.key_pressed`.
- **egui::Key letra→Char** (Task 5): verificar que el mapeo de letras es robusto (Debug-parse o match explícito A..Z); si Debug no da "C", usar match explícito.
- **serde de acción desconocida** (Task 3): la versión simple aborta el parse del Vec → load_keymap cae a defaults global (aceptable); la robusta parsea entrada-por-entrada tolerante. Decidir y documentar.
- **Captura vs handle_input** (Task 6): `process_shortcut_capture` corre ANTES y handle_input retorna temprano mientras se captura, para que la tecla capturada no dispare también su acción.
- **i18n parity** (Task 6): quitar las claves `shortcut.*` viejas de AMBOS json si se dejan de usar; agregar `action.*`/`settings.shortcuts.*` a ambos idénticas.
- **egui::Event::Key shape** (Task 5/6): verificar campos en 0.34.3 (`key`, `pressed`, `repeat`, `modifiers`, `physical_key`).
```
