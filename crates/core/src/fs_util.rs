// Naygo — escritura atómica de archivos (tmp + rename) para no corromper ante crash.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Escribe archivos de forma ATÓMICA: primero a `<nombre>.tmp` en el mismo
//! directorio (→ mismo volumen) y luego `rename` sobre el destino, que en el mismo
//! volumen es una operación atómica. Un crash a media escritura deja, a lo sumo, un
//! `.tmp` huérfano: el archivo real jamás queda truncado ni a medias. Crítico para
//! settings/workspace y para el journal de operaciones (que ES el mecanismo de
//! recuperación ante crashes).

use std::path::Path;

/// Escribe `contents` en `path` de forma atómica (tmp + rename). Si la escritura o
/// el rename fallan, limpia el `.tmp` best-effort y devuelve el error.
pub fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    // `<nombre>.tmp` AL LADO del destino (mismo directorio = mismo volumen: el
    // rename es atómico). Se construye sobre el nombre completo, no con
    // `with_extension` (que reemplazaría el ".json").
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp_name);
    let result = std::fs::write(&tmp, contents).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        // No dejar el .tmp huérfano si el rename no llegó a hacerse (best-effort).
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escribe_contenido_y_no_deja_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        write_atomic(&path, "{\"version\":3}").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"version\":3}");
        assert!(
            !dir.path().join("settings.json.tmp").exists(),
            "el .tmp no debe quedar tras una escritura exitosa"
        );
    }

    #[test]
    fn sobrescribe_un_archivo_existente() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        std::fs::write(&path, b"viejo").unwrap();
        write_atomic(&path, "nuevo").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "nuevo");
        assert!(!dir.path().join("workspace.json.tmp").exists());
    }

    #[test]
    fn crea_el_directorio_padre_antes_de_escribir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config").join("favorites.json");
        write_atomic(&path, "{}").unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{}");
    }

    #[test]
    fn si_falla_limpia_el_tmp() {
        let dir = tempfile::tempdir().unwrap();
        // Un archivo ocupa el lugar del directorio padre: no debe quedar un `.tmp`.
        let parent = dir.path().join("archivo");
        std::fs::write(&parent, b"bloqueo").unwrap();
        let path = parent.join("x.json");
        let r = write_atomic(&path, "contenido");
        assert!(r.is_err());
        assert!(!path.exists());
        assert!(!parent.join("x.json.tmp").exists());
    }
}
