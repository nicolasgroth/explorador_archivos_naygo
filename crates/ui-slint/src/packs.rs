// Naygo — import/export de "packs" (idiomas, temas y configuración) como archivos .zip.
// Un pack es un .zip con uno o más JSON del directorio de configuración:
//   - idioma:  lang/<code>.json
//   - tema:    themes/<id>.json
//   - config:  settings.json (+ keybindings.json si existe)
// El import detecta el tipo por la(s) entrada(s) del zip y extrae a la subcarpeta correcta del
// directorio de configuración. Funciones puras (toman rutas explícitas) → testeables.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;

/// Qué se importó (para que la UI recargue el catálogo correcto).
#[derive(Debug, PartialEq, Eq)]
pub enum ImportKind {
    Lang(String),
    Theme(String),
    Config,
}

fn opts() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)
}

/// Escribe un .zip en `out_zip` con cada (nombre-en-zip, ruta-en-disco). Omite las que no
/// existen. Error si no se puede crear el zip o no había nada que empaquetar.
fn zip_files(out_zip: &Path, entries: &[(String, std::path::PathBuf)]) -> Result<(), String> {
    let file =
        std::fs::File::create(out_zip).map_err(|e| format!("no se pudo crear el zip: {e}"))?;
    let mut zw = zip::ZipWriter::new(file);
    let mut wrote = 0;
    for (name, path) in entries {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        zw.start_file(name.clone(), opts())
            .map_err(|e| format!("error al escribir el zip: {e}"))?;
        zw.write_all(&bytes)
            .map_err(|e| format!("error al escribir {name}: {e}"))?;
        wrote += 1;
    }
    zw.finish()
        .map_err(|e| format!("error al cerrar el zip: {e}"))?;
    if wrote == 0 {
        let _ = std::fs::remove_file(out_zip);
        return Err("no había archivos para exportar".to_string());
    }
    Ok(())
}

/// Exporta el idioma `code` (lang/<code>.json) a `out_zip`.
pub fn export_lang(config_dir: &Path, code: &str, out_zip: &Path) -> Result<(), String> {
    let rel = format!("lang/{code}.json");
    zip_files(out_zip, &[(rel.clone(), config_dir.join(&rel))])
}

/// Exporta el tema `id` (themes/<id>.json) a `out_zip`. Los temas embebidos no tienen archivo
/// en disco; en ese caso el export falla con un mensaje claro (el usuario solo puede exportar
/// temas que viven como archivo en su carpeta de config).
pub fn export_theme(config_dir: &Path, id: &str, out_zip: &Path) -> Result<(), String> {
    let rel = format!("themes/{id}.json");
    let src = config_dir.join(&rel);
    if !src.exists() {
        return Err(format!(
            "el tema «{id}» es embebido (no hay archivo que exportar)"
        ));
    }
    zip_files(out_zip, &[(rel, src)])
}

/// Exporta la configuración (settings.json + keybindings.json si existe) a `out_zip`.
pub fn export_config(config_dir: &Path, out_zip: &Path) -> Result<(), String> {
    zip_files(
        out_zip,
        &[
            (
                "settings.json".to_string(),
                config_dir.join("settings.json"),
            ),
            (
                "keybindings.json".to_string(),
                config_dir.join("keybindings.json"),
            ),
        ],
    )
}

/// Importa un pack `.zip`: detecta el tipo por sus entradas y extrae cada archivo a la
/// subcarpeta correcta de `config_dir`. Valida que cada JSON parsee (no escribe basura).
/// Devuelve qué tipo se importó para que la UI recargue el catálogo.
pub fn import_zip(config_dir: &Path, in_zip: &Path) -> Result<ImportKind, String> {
    let file = std::fs::File::open(in_zip).map_err(|e| format!("no se pudo abrir el zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip inválido: {e}"))?;

    let mut kind: Option<ImportKind> = None;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("entrada {i} ilegible: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        // Solo aceptamos rutas conocidas y sin escapes (../).
        if name.contains("..") {
            return Err(format!("ruta sospechosa en el pack: {name}"));
        }
        let mut contents = String::new();
        entry
            .read_to_string(&mut contents)
            .map_err(|e| format!("no se pudo leer {name}: {e}"))?;
        // Validar que es JSON.
        serde_json::from_str::<serde_json::Value>(&contents)
            .map_err(|_| format!("{name} no es JSON válido"))?;

        let (subdir, this_kind) = classify(&name)?;
        let dest_dir = config_dir.join(subdir);
        std::fs::create_dir_all(&dest_dir)
            .map_err(|e| format!("no se pudo crear {}: {e}", dest_dir.display()))?;
        let dest = config_dir.join(&name);
        std::fs::write(&dest, contents.as_bytes())
            .map_err(|e| format!("no se pudo escribir {}: {e}", dest.display()))?;
        // El primer archivo decide el tipo informado (un pack de config trae 2).
        if kind.is_none() {
            kind = Some(this_kind);
        }
    }
    kind.ok_or_else(|| "el pack está vacío o no tiene archivos reconocidos".to_string())
}

/// Mapea el nombre de una entrada del zip a (subcarpeta, tipo). Error si no se reconoce.
fn classify(name: &str) -> Result<(&'static str, ImportKind), String> {
    if let Some(rest) = name.strip_prefix("lang/") {
        if let Some(code) = rest.strip_suffix(".json") {
            return Ok(("lang", ImportKind::Lang(code.to_string())));
        }
    }
    if let Some(rest) = name.strip_prefix("themes/") {
        if let Some(id) = rest.strip_suffix(".json") {
            return Ok(("themes", ImportKind::Theme(id.to_string())));
        }
    }
    if name == "settings.json" || name == "keybindings.json" {
        return Ok(("", ImportKind::Config));
    }
    Err(format!("entrada no reconocida en el pack: {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_e_import_idioma_round_trip() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        // Crear lang/xx.json en el dir origen.
        std::fs::create_dir_all(src.path().join("lang")).unwrap();
        std::fs::write(src.path().join("lang/xx.json"), r#"{"app.loading":"…"}"#).unwrap();
        let zip_path = src.path().join("pack.zip");

        export_lang(src.path(), "xx", &zip_path).unwrap();
        assert!(zip_path.exists());

        let kind = import_zip(dst.path(), &zip_path).unwrap();
        assert_eq!(kind, ImportKind::Lang("xx".to_string()));
        assert!(dst.path().join("lang/xx.json").exists());
    }

    #[test]
    fn export_tema_inexistente_falla() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("t.zip");
        // dark-blue es embebido (no hay archivo) → debe fallar limpio.
        let err = export_theme(dir.path(), "dark-blue", &zip_path).unwrap_err();
        assert!(err.contains("embebido"));
        assert!(!zip_path.exists());
    }

    #[test]
    fn import_zip_invalido_falla() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.zip");
        std::fs::write(&bad, b"esto no es un zip").unwrap();
        assert!(import_zip(dir.path(), &bad).is_err());
    }

    #[test]
    fn import_json_invalido_en_pack_falla() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        // Empaquetar a mano un themes/x.json que NO es JSON.
        let zip_path = src.path().join("p.zip");
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        zw.start_file("themes/x.json", opts()).unwrap();
        zw.write_all(b"no json").unwrap();
        zw.finish().unwrap();

        assert!(import_zip(dst.path(), &zip_path).is_err());
        // No debe haber escrito el archivo inválido.
        assert!(!dst.path().join("themes/x.json").exists());
    }
}
