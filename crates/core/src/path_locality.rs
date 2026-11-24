// Naygo — clasificación local/remota de rutas para repartir el cupo de listados.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Un share de red colgado retiene su hilo de listado (y su plaza) hasta que el sistema
//! desiste, a veces minutos. Para que esos hilos no dejen sin cupo a los discos locales, la UI
//! reparte los listados en dos admisiones según la ruta. Este módulo decide de qué lado cae
//! cada ruta sin tocar el disco:
//!
//! - Las rutas UNC (`\\servidor\recurso`, `\\?\UNC\servidor\recurso`) son remotas por sintaxis.
//!   No se consulta `GetDriveTypeW` para ellas: sobre una raíz UNC esa llamada sí puede ir a la red.
//! - Para una letra de unidad (o un punto de montaje fuera de Windows), el tipo lo aporta
//!   `VolumeLocality`, que implementa `naygo-platform` con la tabla de unidades que ya enumeró
//!   su worker. Core no conoce Win32.
//! - Un volumen que la tabla no conoce (la tabla aún no llegó al arrancar, una unidad recién
//!   mapeada) se trata como remoto. El error conservador es el barato: un listado local de más en
//!   el cupo remoto solo espera su turno, mientras que un share colgado contado como local volvería
//!   a dejar sin cupo a los discos locales.

use std::path::Path;

/// Consulta inyectada sobre el tipo de volumen de una ruta.
pub trait VolumeLocality {
    /// `Some(true)` si el volumen es remoto, `Some(false)` si es local y `None` si no figura en
    /// la tabla. No debe hacer I/O: se llama desde el hilo de UI.
    fn volume_is_remote(&self, path: &Path) -> Option<bool>;
}

/// Clasificador sin tabla: todo volumen con letra es desconocido. Sirve para pruebas y para
/// llamadores que solo necesitan la regla sintáctica UNC.
pub struct UnknownVolumes;

impl VolumeLocality for UnknownVolumes {
    fn volume_is_remote(&self, _path: &Path) -> Option<bool> {
        None
    }
}

/// `true` si la ruta debe usar el cupo remoto de listados.
pub fn is_remote_path(path: &Path, locality: &dyn VolumeLocality) -> bool {
    is_unc_path(path) || locality.volume_is_remote(path).unwrap_or(true)
}

/// `true` si la ruta es UNC, incluida la forma extendida `\\?\UNC\` (y `\\.\UNC\`).
/// Las formas `\\?\C:\` y `\\.\C:\` no son UNC: apuntan a una unidad con letra.
pub fn is_unc_path(path: &Path) -> bool {
    let text = path.as_os_str().to_string_lossy();
    let bytes = text.as_bytes();
    if bytes.len() < 2 || !is_separator(bytes[0]) || !is_separator(bytes[1]) {
        return false;
    }
    match device_prefix_rest(bytes) {
        Some(rest) => {
            rest.len() >= 4 && rest[..3].eq_ignore_ascii_case(b"UNC") && is_separator(rest[3])
        }
        None => bytes.len() > 2 && !is_separator(bytes[2]),
    }
}

/// Letra de unidad de la ruta, en mayúscula (`C:\x`, `\\?\C:\x` y `\\.\C:\x` dan `C`).
pub fn drive_letter(path: &Path) -> Option<char> {
    let text = path.as_os_str().to_string_lossy();
    let bytes = text.as_bytes();
    let rest = device_prefix_rest(bytes).unwrap_or(bytes);
    match rest {
        [letter, b':', ..] if letter.is_ascii_alphabetic() => {
            Some(char::from(letter.to_ascii_uppercase()))
        }
        _ => None,
    }
}

/// Resto de la ruta tras `\\?\` o `\\.\`; `None` si no tiene ese prefijo.
fn device_prefix_rest(bytes: &[u8]) -> Option<&[u8]> {
    match bytes {
        [a, b, b'?' | b'.', c, rest @ ..]
            if is_separator(*a) && is_separator(*b) && is_separator(*c) =>
        {
            Some(rest)
        }
        _ => None,
    }
}

/// Separadores de ruta con valor sintáctico en Windows. Fuera de Windows `/` es un separador de
/// rutas POSIX (`//x` es una ruta absoluta), así que allí solo la barra invertida cuenta.
fn is_separator(byte: u8) -> bool {
    byte == b'\\' || (cfg!(windows) && byte == b'/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Table(Vec<(char, bool)>);

    impl VolumeLocality for Table {
        fn volume_is_remote(&self, path: &Path) -> Option<bool> {
            let letter = drive_letter(path)?;
            self.0
                .iter()
                .find(|(known, _)| *known == letter)
                .map(|(_, remote)| *remote)
        }
    }

    #[test]
    fn unc_y_su_forma_extendida_son_remotas_por_sintaxis() {
        for path in [
            r"\\servidor\recurso",
            r"\\servidor\recurso\carpeta",
            r"\\?\UNC\servidor\recurso\carpeta",
            r"\\?\unc\servidor\recurso",
            r"\\.\UNC\servidor\recurso",
        ] {
            assert!(is_unc_path(Path::new(path)), "{path}");
            assert!(
                is_remote_path(Path::new(path), &Table(Vec::new())),
                "{path}"
            );
        }
    }

    #[test]
    fn rutas_con_letra_y_extendidas_de_disco_no_son_unc() {
        for path in [
            r"C:\Users",
            r"\\?\C:\Users",
            r"\\.\C:\Users",
            r"\\?\Volume{0000}\x",
            r"\\?\UNCX\x",
            "relativa",
        ] {
            assert!(!is_unc_path(Path::new(path)), "{path}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn unc_con_barras_normales_en_windows() {
        assert!(is_unc_path(Path::new("//servidor/recurso")));
    }

    #[test]
    fn letra_de_unidad_ignora_el_prefijo_extendido() {
        assert_eq!(drive_letter(Path::new(r"c:\x")), Some('C'));
        assert_eq!(drive_letter(Path::new(r"\\?\D:\x")), Some('D'));
        assert_eq!(drive_letter(Path::new(r"\\.\e:\")), Some('E'));
        assert_eq!(drive_letter(Path::new(r"\\servidor\recurso")), None);
        assert_eq!(drive_letter(&PathBuf::from("relativa")), None);
    }

    #[test]
    fn la_tabla_decide_las_letras_y_lo_desconocido_cuenta_como_remoto() {
        let table = Table(vec![('C', false), ('Z', true)]);
        assert!(!is_remote_path(Path::new(r"C:\Users"), &table));
        assert!(!is_remote_path(Path::new(r"\\?\C:\Users"), &table));
        assert!(is_remote_path(Path::new(r"Z:\equipo"), &table));
        assert!(is_remote_path(Path::new(r"Q:\sin-tabla"), &table));
        assert!(is_remote_path(Path::new(r"C:\x"), &UnknownVolumes));
    }
}
