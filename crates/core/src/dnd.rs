// Naygo — lógica pura de drag & drop: decidir mover vs copiar.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Decide si un drop mueve o copia, con las reglas del Explorador de Windows:
//! Shift = mover, Ctrl = copiar, sin tecla = mover en el mismo disco / copiar entre
//! discos distintos. Puro y testeable; la capa UI lee los modificadores y los discos.

use std::path::Path;

/// Acción resultante de un drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropAction {
    Move,
    Copy,
}

/// Decide la acción según modificadores y si origen/destino están en el mismo disco.
/// Prioridad: Shift→Move, Ctrl→Copy, si no: same_drive→Move, distinto→Copy.
/// (Si ambos modificadores, Shift gana — coincide con el comportamiento de Windows.)
pub fn decide_drop_action(ctrl: bool, shift: bool, same_drive: bool) -> DropAction {
    if shift {
        DropAction::Move
    } else if ctrl {
        DropAction::Copy
    } else if same_drive {
        DropAction::Move
    } else {
        DropAction::Copy
    }
}

/// ¿`a` y `b` están en el mismo disco/volumen? En Windows compara la letra de unidad
/// (case-insensitive). Si alguna no tiene prefijo de unidad reconocible, devuelve
/// `false` (conservador: trata como discos distintos → copiar, más seguro que mover).
pub fn same_drive(a: &Path, b: &Path) -> bool {
    fn drive_letter(p: &Path) -> Option<char> {
        let s = p.to_string_lossy();
        let mut chars = s.chars();
        let c = chars.next()?;
        if c.is_ascii_alphabetic() && chars.next() == Some(':') {
            Some(c.to_ascii_uppercase())
        } else {
            None
        }
    }
    match (drive_letter(a), drive_letter(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn shift_siempre_mueve() {
        assert_eq!(decide_drop_action(false, true, false), DropAction::Move);
        assert_eq!(decide_drop_action(true, true, false), DropAction::Move);
    }
    #[test]
    fn ctrl_copia() {
        assert_eq!(decide_drop_action(true, false, true), DropAction::Copy);
    }
    #[test]
    fn sin_tecla_mismo_disco_mueve() {
        assert_eq!(decide_drop_action(false, false, true), DropAction::Move);
    }
    #[test]
    fn sin_tecla_distinto_disco_copia() {
        assert_eq!(decide_drop_action(false, false, false), DropAction::Copy);
    }
    #[test]
    fn same_drive_misma_letra() {
        assert!(same_drive(
            &PathBuf::from("C:\\a"),
            &PathBuf::from("C:\\b\\c")
        ));
        assert!(same_drive(&PathBuf::from("c:\\a"), &PathBuf::from("C:\\b")));
    }
    #[test]
    fn same_drive_distinta_letra() {
        assert!(!same_drive(
            &PathBuf::from("C:\\a"),
            &PathBuf::from("D:\\b")
        ));
    }
    #[test]
    fn same_drive_sin_letra_es_false() {
        assert!(!same_drive(
            &PathBuf::from("\\\\red\\share"),
            &PathBuf::from("C:\\a")
        ));
    }
}
