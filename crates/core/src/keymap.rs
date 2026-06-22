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
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    /// Avanzar una página (PageDown).
    PageDown,
    /// Retroceder una página (PageUp).
    PageUp,
    /// Ir al inicio de la lista (Home).
    Home,
    /// Ir al final de la lista (End).
    End,
    /// Barra espaciadora.
    Space,
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
    /// Ir a la carpeta de inicio configurada (Home). Alt+Home.
    GoHome,
    SwitchPane,
    CancelListing,
    Copy,
    Cut,
    Paste,
    Delete,
    DeletePermanent,
    Rename,
    /// Renombrado por lotes: abre la ventana de batch-rename con la selección (Shift+F2).
    BatchRename,
    NewFile,
    NewDir,
    CopyToOther,
    MoveToOther,
    /// Refrescar (re-listar) la carpeta del panel activo — estilo navegador (F5).
    Refresh,
    /// Buscar archivos por nombre en la carpeta del panel activo y todas sus subcarpetas (Ctrl+F).
    Find,
    /// Calcular el tamaño de la carpeta enfocada/seleccionada (fase sizing).
    ComputeSize,
    /// Seleccionar todos los ítems de la vista (fase multi-selección).
    SelectAll,
    /// Extender la selección hacia arriba desde el ancla (Shift+↑).
    ExtendUp,
    /// Extender la selección hacia abajo desde el ancla (Shift+↓).
    ExtendDown,
    /// Marcar/desmarcar el ítem enfocado (Espacio).
    ToggleSelect,
    /// Bajar una página el foco (PageDown) — selección simple del nuevo foco.
    FocusPageDown,
    /// Subir una página el foco (PageUp) — selección simple del nuevo foco.
    FocusPageUp,
    /// Foco al primer ítem (Home) — selección simple.
    FocusHome,
    /// Foco al último ítem (End) — selección simple.
    FocusEnd,
    /// Extender la selección una página hacia abajo (Shift+PageDown).
    ExtendPageDown,
    /// Extender la selección una página hacia arriba (Shift+PageUp).
    ExtendPageUp,
    /// Extender la selección hasta el primer ítem (Shift+Home).
    ExtendHome,
    /// Extender la selección hasta el último ítem (Shift+End).
    ExtendEnd,
    /// Mover el foco hacia arriba SIN tocar la selección (Ctrl+↑).
    FocusUpKeep,
    /// Mover el foco hacia abajo SIN tocar la selección (Ctrl+↓).
    FocusDownKeep,
    /// Marcar/desmarcar el ítem enfocado dejando el resto (Ctrl+Espacio).
    ToggleFocused,
    /// Deshacer la última operación de archivos (R2).
    Undo,
    /// Abrir la edición del path del panel activo (path-bar, Ctrl+L / F4).
    EditPath,
    /// Abrir/cerrar la ayuda (F1): overlay con las secciones y los atajos activos.
    Help,
    /// Navegar el panel activo al favorito N (Ctrl+1..Ctrl+9).
    GoFavorite1,
    GoFavorite2,
    GoFavorite3,
    GoFavorite4,
    GoFavorite5,
    GoFavorite6,
    GoFavorite7,
    GoFavorite8,
    GoFavorite9,
    /// Abrir la paleta de comandos (Ctrl+P).
    CommandPalette,
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
            GoHome,
            SwitchPane,
            CancelListing,
            Copy,
            Cut,
            Paste,
            Delete,
            DeletePermanent,
            Rename,
            BatchRename,
            NewFile,
            NewDir,
            CopyToOther,
            MoveToOther,
            Refresh,
            Find,
            ComputeSize,
            SelectAll,
            ExtendUp,
            ExtendDown,
            ToggleSelect,
            FocusPageDown,
            FocusPageUp,
            FocusHome,
            FocusEnd,
            ExtendPageDown,
            ExtendPageUp,
            ExtendHome,
            ExtendEnd,
            FocusUpKeep,
            FocusDownKeep,
            ToggleFocused,
            Undo,
            EditPath,
            Help,
            GoFavorite1,
            GoFavorite2,
            GoFavorite3,
            GoFavorite4,
            GoFavorite5,
            GoFavorite6,
            GoFavorite7,
            GoFavorite8,
            GoFavorite9,
            CommandPalette,
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
            GoHome => "action.go_home",
            SwitchPane => "action.switch_pane",
            CancelListing => "action.cancel_listing",
            Copy => "action.copy",
            Cut => "action.cut",
            Paste => "action.paste",
            Delete => "action.delete",
            DeletePermanent => "action.delete_permanent",
            Rename => "action.rename",
            BatchRename => "action.batch_rename",
            NewFile => "action.new_file",
            NewDir => "action.new_dir",
            CopyToOther => "action.copy_to_other",
            MoveToOther => "action.move_to_other",
            Refresh => "action.refresh",
            Find => "action.find",
            ComputeSize => "action.compute_size",
            SelectAll => "action.select_all",
            ExtendUp => "action.extend_up",
            ExtendDown => "action.extend_down",
            Undo => "action.undo",
            ToggleSelect => "action.toggle_select",
            FocusPageDown => "action.focus_page_down",
            FocusPageUp => "action.focus_page_up",
            FocusHome => "action.focus_home",
            FocusEnd => "action.focus_end",
            ExtendPageDown => "action.extend_page_down",
            ExtendPageUp => "action.extend_page_up",
            ExtendHome => "action.extend_home",
            ExtendEnd => "action.extend_end",
            FocusUpKeep => "action.focus_up_keep",
            FocusDownKeep => "action.focus_down_keep",
            ToggleFocused => "action.toggle_focused",
            EditPath => "action.edit_path",
            Help => "action.help",
            GoFavorite1 => "action.go_favorite_1",
            GoFavorite2 => "action.go_favorite_2",
            GoFavorite3 => "action.go_favorite_3",
            GoFavorite4 => "action.go_favorite_4",
            GoFavorite5 => "action.go_favorite_5",
            GoFavorite6 => "action.go_favorite_6",
            GoFavorite7 => "action.go_favorite_7",
            GoFavorite8 => "action.go_favorite_8",
            GoFavorite9 => "action.go_favorite_9",
            CommandPalette => "action.command_palette",
        }
    }

    /// Si la acción es `GoFavoriteN`, su índice 0-based en la lista de favoritos.
    pub fn favorite_index(self) -> Option<usize> {
        use Action::*;
        match self {
            GoFavorite1 => Some(0),
            GoFavorite2 => Some(1),
            GoFavorite3 => Some(2),
            GoFavorite4 => Some(3),
            GoFavorite5 => Some(4),
            GoFavorite6 => Some(5),
            GoFavorite7 => Some(6),
            GoFavorite8 => Some(7),
            GoFavorite9 => Some(8),
            _ => None,
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
            // Home: ir a la carpeta de inicio. Alt+Home (Home a secas es FocusHome).
            (GoHome, vec![Chord::alt(Home)]),
            (SwitchPane, vec![Chord::plain(Tab)]),
            (CancelListing, vec![Chord::plain(Escape)]),
            (Copy, vec![Chord::ctrl(Char('c'))]),
            (Cut, vec![Chord::ctrl(Char('x'))]),
            (Paste, vec![Chord::ctrl(Char('v'))]),
            (Action::Delete, vec![Chord::plain(KeyCode::Delete)]),
            (DeletePermanent, vec![Chord::shift(KeyCode::Delete)]),
            (Rename, vec![Chord::plain(F2)]),
            (BatchRename, vec![Chord::shift(F2)]),
            (NewFile, vec![Chord::ctrl(Char('n'))]),
            (NewDir, vec![Chord::ctrl_shift(Char('n'))]),
            // F5 es REFRESCAR (estilo navegador), decisión de Nicolás. CopyToOther queda sin
            // atajo por defecto (asignable en el editor); MoveToOther conserva F6.
            (CopyToOther, vec![]),
            (MoveToOther, vec![Chord::plain(F6)]),
            (Refresh, vec![Chord::plain(F5)]),
            (Find, vec![Chord::ctrl(Char('f'))]),
            (ComputeSize, vec![Chord::plain(F3)]),
            (SelectAll, vec![Chord::ctrl(Char('a'))]),
            (ExtendUp, vec![Chord::shift(ArrowUp)]),
            (ExtendDown, vec![Chord::shift(ArrowDown)]),
            (ToggleSelect, vec![Chord::plain(Space)]),
            // Navegación por bloques (estilo Explorer): página, inicio y fin.
            (FocusPageDown, vec![Chord::plain(PageDown)]),
            (FocusPageUp, vec![Chord::plain(PageUp)]),
            (FocusHome, vec![Chord::plain(Home)]),
            (FocusEnd, vec![Chord::plain(End)]),
            // Mismas teclas con Shift extienden la selección por bloques.
            (ExtendPageDown, vec![Chord::shift(PageDown)]),
            (ExtendPageUp, vec![Chord::shift(PageUp)]),
            (ExtendHome, vec![Chord::shift(Home)]),
            (ExtendEnd, vec![Chord::shift(End)]),
            // Ctrl+flechas mueven el foco SIN tocar la selección.
            (FocusUpKeep, vec![Chord::ctrl(ArrowUp)]),
            (FocusDownKeep, vec![Chord::ctrl(ArrowDown)]),
            // Ctrl+Espacio togglea el ítem enfocado dejando el resto.
            (ToggleFocused, vec![Chord::ctrl(Space)]),
            (Undo, vec![Chord::ctrl(Char('z'))]),
            // Path-bar: dos atajos (estilo navegador y estilo Explorer/Commander).
            (EditPath, vec![Chord::ctrl(Char('l')), Chord::plain(F4)]),
            (Help, vec![Chord::plain(F1)]),
            // Favoritos: Ctrl+dígito salta al favorito N (orden del panel).
            (GoFavorite1, vec![Chord::ctrl(Char('1'))]),
            (GoFavorite2, vec![Chord::ctrl(Char('2'))]),
            (GoFavorite3, vec![Chord::ctrl(Char('3'))]),
            (GoFavorite4, vec![Chord::ctrl(Char('4'))]),
            (GoFavorite5, vec![Chord::ctrl(Char('5'))]),
            (GoFavorite6, vec![Chord::ctrl(Char('6'))]),
            (GoFavorite7, vec![Chord::ctrl(Char('7'))]),
            (GoFavorite8, vec![Chord::ctrl(Char('8'))]),
            (GoFavorite9, vec![Chord::ctrl(Char('9'))]),
            // Paleta de comandos: Ctrl+P abre el buscador de acciones.
            (CommandPalette, vec![Chord::ctrl(Char('p'))]),
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
    fn all_tiene_53_acciones_con_clave_i18n_unica() {
        let all = Action::all();
        assert_eq!(all.len(), 53);
        let mut keys: Vec<&str> = all.iter().map(|a| a.i18n_key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 53, "cada acción tiene una clave i18n única");
    }

    #[test]
    fn ctrl_p_dispara_la_paleta() {
        let km = KeyMap::defaults();
        let chord = Chord::ctrl(KeyCode::Char('p'));
        assert_eq!(km.action_for(&chord), Some(Action::CommandPalette));
    }

    #[test]
    fn alt_home_dispara_go_home() {
        let km = KeyMap::defaults();
        let chord = Chord::alt(KeyCode::Home);
        assert_eq!(km.action_for(&chord), Some(Action::GoHome));
    }

    #[test]
    fn teclado_de_lista_por_bloques() {
        let km = KeyMap::defaults();
        // Navegación por bloques sin modificador.
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::PageDown)),
            Some(Action::FocusPageDown)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::PageUp)),
            Some(Action::FocusPageUp)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::Home)),
            Some(Action::FocusHome)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::End)),
            Some(Action::FocusEnd)
        );
        // Shift extiende la selección por bloques.
        assert_eq!(
            km.action_for(&Chord::shift(KeyCode::PageDown)),
            Some(Action::ExtendPageDown)
        );
        assert_eq!(
            km.action_for(&Chord::shift(KeyCode::End)),
            Some(Action::ExtendEnd)
        );
        // Ctrl+flechas mueven el foco sin tocar la selección; Ctrl+Espacio togglea.
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::ArrowUp)),
            Some(Action::FocusUpKeep)
        );
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::ArrowDown)),
            Some(Action::FocusDownKeep)
        );
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Space)),
            Some(Action::ToggleFocused)
        );
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
            Some(Action::Refresh),
            "F5 = refrescar (estilo navegador)"
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::F6)),
            Some(Action::MoveToOther)
        );
        assert_eq!(
            km.chords_for(Action::CopyToOther),
            &[],
            "CopyToOther sin atajo por defecto (F5 liberado para refrescar)"
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
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('a'))),
            Some(Action::SelectAll)
        );
        assert_eq!(
            km.action_for(&Chord::shift(KeyCode::ArrowUp)),
            Some(Action::ExtendUp)
        );
        assert_eq!(
            km.action_for(&Chord::shift(KeyCode::ArrowDown)),
            Some(Action::ExtendDown)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::Space)),
            Some(Action::ToggleSelect)
        );
    }

    #[test]
    fn edit_path_tiene_ctrl_l_y_f4() {
        let km = KeyMap::defaults();
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('l'))),
            Some(Action::EditPath)
        );
        assert_eq!(
            km.action_for(&Chord::plain(KeyCode::F4)),
            Some(Action::EditPath)
        );
        assert_eq!(km.chords_for(Action::EditPath).len(), 2);
    }

    #[test]
    fn favoritos_ctrl_digito_y_sin_choques() {
        let km = KeyMap::defaults();
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('1'))),
            Some(Action::GoFavorite1)
        );
        assert_eq!(
            km.action_for(&Chord::ctrl(KeyCode::Char('9'))),
            Some(Action::GoFavorite9)
        );
        // Cada acción de favoritos tiene EXACTAMENTE un chord por defecto: si otro
        // default se lo hubiese robado (choque), este conteo lo delataría.
        for (i, a) in [
            Action::GoFavorite1,
            Action::GoFavorite2,
            Action::GoFavorite3,
            Action::GoFavorite4,
            Action::GoFavorite5,
            Action::GoFavorite6,
            Action::GoFavorite7,
            Action::GoFavorite8,
            Action::GoFavorite9,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(km.chords_for(a).len(), 1, "GoFavorite{} sin chord", i + 1);
            assert_eq!(a.favorite_index(), Some(i));
        }
        assert_eq!(Action::Copy.favorite_index(), None);
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
        assert_eq!(km.action_for(&Chord::ctrl(KeyCode::Char('j'))), None);
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
        let r = km.bind(Action::Copy, Chord::ctrl(KeyCode::Char('j')));
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
