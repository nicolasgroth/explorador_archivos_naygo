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
        Chord {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }
    /// Con Ctrl.
    pub fn ctrl(key: KeyCode) -> Chord {
        Chord {
            key,
            ctrl: true,
            shift: false,
            alt: false,
        }
    }
    /// Con Ctrl+Shift.
    pub fn ctrl_shift(key: KeyCode) -> Chord {
        Chord {
            key,
            ctrl: true,
            shift: true,
            alt: false,
        }
    }
    /// Con Shift.
    pub fn shift(key: KeyCode) -> Chord {
        Chord {
            key,
            ctrl: false,
            shift: true,
            alt: false,
        }
    }
    /// Con Alt.
    pub fn alt(key: KeyCode) -> Chord {
        Chord {
            key,
            ctrl: false,
            shift: false,
            alt: true,
        }
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
            ComputeSize,
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
            (Open, vec![]),
            (OpenWith, vec![]),
            (GoUp, vec![Chord::plain(Backspace), Chord::plain(ArrowLeft)]),
            (GoBack, vec![Chord::alt(ArrowLeft)]),
            (GoForward, vec![Chord::alt(ArrowRight)]),
            (SwitchPane, vec![Chord::plain(Tab)]),
            (CancelListing, vec![Chord::plain(Escape)]),
            (Copy, vec![Chord::ctrl(Char('c'))]),
            (Cut, vec![Chord::ctrl(Char('x'))]),
            (Paste, vec![Chord::ctrl(Char('v'))]),
            (Action::Delete, vec![Chord::plain(KeyCode::Delete)]),
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

    /// Asigna `chord` a `action`. Si el chord ya era de OTRA acción, se lo quita y devuelve
    /// esa otra acción (conflicto reasignado). Si ya era de `action`, no-op (None).
    pub fn bind(&mut self, action: Action, chord: Chord) -> Option<Action> {
        match self.action_for(&chord) {
            Some(a) if a == action => None,
            Some(a) => {
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

    /// Restaura los atajos por defecto de UNA acción (quitándolos de quien los tenga).
    pub fn reset_action(&mut self, action: Action) {
        let def = KeyMap::defaults();
        let default_chords = def.chords_for(action).to_vec();
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

    /// Mergea lo almacenado con los defaults: arranca de defaults, y por cada entrada con
    /// una acción conocida reemplaza sus chords. (Las acciones sin entrada conservan su
    /// default; eso da retro-compat ante acciones nuevas como ComputeSize.)
    fn from_stored(stored: Vec<StoredBinding>) -> KeyMap {
        let mut km = KeyMap::defaults();
        for entry in stored {
            if let Some(pos) = km.bindings.iter().position(|(a, _)| *a == entry.action) {
                km.bindings[pos].1 = entry.chords;
            }
        }
        km
    }
}

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
            .map(|(a, c)| StoredBinding {
                action: *a,
                chords: c.clone(),
            })
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
        assert_eq!(
            Chord::plain(KeyCode::F3),
            Chord {
                key: KeyCode::F3,
                ctrl: false,
                shift: false,
                alt: false
            }
        );
        assert!(Chord::ctrl(KeyCode::Char('c')).ctrl);
        assert!(Chord::ctrl_shift(KeyCode::Char('n')).shift);
    }

    #[test]
    fn defaults_atajos_clave() {
        let km = KeyMap::defaults();
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('c'))),
            Some(Action::Copy)
        );
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('x'))),
            Some(Action::Cut)
        );
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('v'))),
            Some(Action::Paste)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::F2)),
            Some(Action::Rename)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::F3)),
            Some(Action::ComputeSize)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::F5)),
            Some(Action::CopyToOther)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::F6)),
            Some(Action::MoveToOther)
        );
        assert_eq!(
            km.action_for(&Chord::ctrl_shift(KeyCode::Char('n'))),
            Some(Action::NewDir)
        );
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('n'))),
            Some(Action::NewFile)
        );
        assert_eq!(
            km.action_for(&Chord::shift(KeyCode::Delete)),
            Some(Action::DeletePermanent)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::Delete)),
            Some(Action::Delete)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::Enter)),
            Some(Action::Activate)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::Tab)),
            Some(Action::SwitchPane)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::Escape)),
            Some(Action::CancelListing)
        );
        assert_eq!(
            km.action_for(&Chord::alt(KeyCode::ArrowLeft)),
            Some(Action::GoBack)
        );
        assert_eq!(
            km.action_for(&Chord::alt(KeyCode::ArrowRight)),
            Some(Action::GoForward)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::ArrowUp)),
            Some(Action::MoveUp)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::ArrowDown)),
            Some(Action::MoveDown)
        );
    }

    #[test]
    fn go_up_tiene_dos_atajos() {
        let km = KeyMap::defaults();
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::Backspace)),
            Some(Action::GoUp)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::ArrowLeft)),
            Some(Action::GoUp)
        );
        assert_eq!(km.chords_for(Action::GoUp).len(), 2);
    }

    #[test]
    fn open_sin_atajo_por_defecto() {
        let km = KeyMap::defaults();
        assert!(km.chords_for(Action::Open).is_empty());
        assert!(km.chords_for(Action::OpenWith).is_empty());
    }

    #[test]
    fn action_for_libre_es_none() {
        let km = KeyMap::defaults();
        assert_eq!(km.action_for(&Chord::ctrl(KeyCode::Char('z'))), None);
    }

    #[test]
    fn bind_conflicto_reasigna_y_devuelve_la_despojada() {
        let mut km = KeyMap::defaults();
        let robbed = km.bind(Action::Rename, Chord::ctrl(KeyCode::Char('c')));
        assert_eq!(robbed, Some(Action::Copy));
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('c'))),
            Some(Action::Rename)
        );
        assert!(!km
            .chords_for(Action::Copy)
            .contains(&Chord::ctrl(KeyCode::Char('c'))));
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
        let r = km.bind(Action::Copy, Chord::ctrl(KeyCode::Char('z')));
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
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('c'))),
            Some(Action::Copy)
        );
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
        // Construir el json mergeando: solo Copy con Ctrl+Q; el resto debe caer a default.
        let mut src = KeyMap::defaults();
        // dejar Copy con solo Ctrl+Q:
        src.unbind(Action::Copy, &Chord::ctrl(KeyCode::Char('c')));
        src.bind(Action::Copy, Chord::ctrl(KeyCode::Char('q')));
        let json = serde_json::to_string(&src).unwrap();
        let km: KeyMap = serde_json::from_str(&json).unwrap();
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('q'))),
            Some(Action::Copy)
        );
        // Rename conserva F2 (estaba en el json con su default igual).
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::F2)),
            Some(Action::Rename)
        );
    }

    #[test]
    fn serde_json_parcial_mergea_defaults() {
        // Un json con SOLO una acción (Rename con F2) → las demás toman su default.
        // Usamos serde_json::Value para no depender del formato exacto de Chord.
        let f2 = serde_json::to_value(Chord::plain(KeyCode::F2)).unwrap();
        let entry = serde_json::json!({ "action": "Rename", "chords": [f2] });
        let arr = serde_json::Value::Array(vec![entry]);
        let json = serde_json::to_string(&arr).unwrap();
        let km: KeyMap = serde_json::from_str(&json).unwrap();
        // Rename quedó con F2; Copy conserva su default Ctrl+C (no estaba en el json).
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::F2)),
            Some(Action::Rename)
        );
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('c'))),
            Some(Action::Copy)
        );
    }
}
