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

/// ¿`a` y `b` están en el mismo disco/volumen? En Windows el "volumen" puede ser una letra
/// de unidad (`C:`) o un recurso de red UNC (`\\servidor\recurso`). Se compara una CLAVE de
/// volumen normalizada (`volume_key`), case-insensitive, tolerante a:
/// - prefijos extendidos (`\\?\C:\...`, `\\?\UNC\servidor\recurso`),
/// - separadores `/` o `\`,
/// - mayúsculas/minúsculas (Windows no distingue: `C:` == `c:`).
///
/// Si alguna ruta no tiene una clave de volumen reconocible, devuelve `false` (conservador:
/// se trata como discos distintos → copiar, más seguro que mover por error).
///
/// Nota: esto es heurístico por string (core es portable, no usa Win32). No detecta dos
/// letras montadas sobre el MISMO disco físico (p. ej. particiones) como el mismo volumen;
/// eso es lo correcto para decidir mover vs copiar, porque cada letra es un sistema de
/// archivos distinto y un `rename` entre ellas falla igual que entre discos.
pub fn same_drive(a: &Path, b: &Path) -> bool {
    match (volume_key(a), volume_key(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// Clave de volumen normalizada de una ruta de Windows, o `None` si no se reconoce.
/// Tolera los prefijos extendidos `\\?\` y `\\.\` (incluido `\\?\UNC\...`). Devuelve:
///
/// - Letra de unidad → `"C"` (mayúscula).
/// - Recurso UNC → `"\\\\SERVIDOR\\RECURSO"` (mayúsculas, separadores `\`).
fn volume_key(p: &Path) -> Option<String> {
    let s = p.to_string_lossy();
    // Normalizar separadores a `\` para parsear de forma uniforme.
    let s = s.replace('/', "\\");

    // Quitar el prefijo extendido `\\?\` o `\\.\` si está presente. `\\?\UNC\srv\share`
    // representa un UNC: tras quitar `\\?\UNC\` queda `srv\share`, que tratamos como UNC.
    if let Some(rest) = s
        .strip_prefix("\\\\?\\")
        .or_else(|| s.strip_prefix("\\\\.\\"))
    {
        if let Some(unc) = rest.strip_prefix("UNC\\") {
            return unc_key(unc);
        }
        // Tras el prefijo extendido viene normalmente una letra de unidad: `C:\...`.
        return drive_key(rest);
    }

    // UNC normal: empieza con `\\servidor\recurso`.
    if let Some(unc) = s.strip_prefix("\\\\") {
        return unc_key(unc);
    }

    // Ruta con letra de unidad: `C:\...` o `C:`.
    drive_key(&s)
}

/// Clave de una ruta que empieza con letra de unidad (`C:...`) → `"C"` en mayúscula.
fn drive_key(s: &str) -> Option<String> {
    let mut chars = s.chars();
    let c = chars.next()?;
    if c.is_ascii_alphabetic() && chars.next() == Some(':') {
        Some(c.to_ascii_uppercase().to_string())
    } else {
        None
    }
}

/// Clave de un UNC ya sin el prefijo `\\` (es decir, `servidor\recurso\...`):
/// `"\\\\SERVIDOR\\RECURSO"` en mayúsculas. El "volumen" de un UNC es el par
/// servidor+recurso; el resto de la ruta no influye. `None` si falta servidor o recurso.
fn unc_key(rest: &str) -> Option<String> {
    let mut parts = rest.split('\\').filter(|p| !p.is_empty());
    let server = parts.next()?;
    let share = parts.next()?;
    Some(format!(
        "\\\\{}\\{}",
        server.to_ascii_uppercase(),
        share.to_ascii_uppercase()
    ))
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
    fn same_drive_misma_letra_distinto_case() {
        // Windows no distingue mayúsculas en la letra de unidad: C: == c:.
        assert!(same_drive(
            &PathBuf::from("F:\\Maquinas Virtuales\\x"),
            &PathBuf::from("f:\\otra\\y")
        ));
    }
    #[test]
    fn same_drive_rutas_extendidas() {
        // El prefijo extendido `\\?\C:\...` es la MISMA unidad que `C:\...`.
        assert!(same_drive(
            &PathBuf::from("\\\\?\\C:\\a"),
            &PathBuf::from("C:\\b")
        ));
        assert!(same_drive(
            &PathBuf::from("\\\\?\\c:\\a"),
            &PathBuf::from("\\\\?\\C:\\b\\c")
        ));
    }
    #[test]
    fn same_drive_separadores_mixtos() {
        // Rutas con `/` (a veces aparecen) deben normalizarse igual.
        assert!(same_drive(
            &PathBuf::from("C:/a/b"),
            &PathBuf::from("C:\\c")
        ));
    }
    #[test]
    fn same_drive_unc_mismo_share() {
        // Mismo servidor+recurso UNC = mismo volumen (case-insensitive, ruta interna distinta).
        assert!(same_drive(
            &PathBuf::from("\\\\servidor\\recurso\\a"),
            &PathBuf::from("\\\\SERVIDOR\\Recurso\\b\\c")
        ));
    }
    #[test]
    fn same_drive_unc_extendido() {
        // `\\?\UNC\servidor\recurso\...` es el mismo volumen que `\\servidor\recurso\...`.
        assert!(same_drive(
            &PathBuf::from("\\\\?\\UNC\\servidor\\recurso\\a"),
            &PathBuf::from("\\\\servidor\\recurso\\b")
        ));
    }
    #[test]
    fn same_drive_unc_distinto_share() {
        // Mismo servidor pero distinto recurso = distinto volumen.
        assert!(!same_drive(
            &PathBuf::from("\\\\servidor\\uno\\a"),
            &PathBuf::from("\\\\servidor\\dos\\b")
        ));
    }
    #[test]
    fn same_drive_unc_vs_letra_es_false() {
        // Un UNC y una letra de unidad nunca son el mismo volumen.
        assert!(!same_drive(
            &PathBuf::from("\\\\red\\share"),
            &PathBuf::from("C:\\a")
        ));
    }
    #[test]
    fn same_drive_sin_volumen_es_false() {
        // Una ruta relativa (sin letra ni UNC) no tiene clave de volumen → false (conservador).
        assert!(!same_drive(
            &PathBuf::from("carpeta\\sub"),
            &PathBuf::from("C:\\a")
        ));
    }
}
