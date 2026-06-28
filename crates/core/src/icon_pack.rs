// Naygo — empaquetado de sets de íconos: export/import del archivo .naygoset (zip).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Un `.naygoset` es un zip con `manifest.json` + `icons/` (los PNG que el set
//! aporta). Export toma el set efectivo (base + overrides) y lo empaqueta autocontenido;
//! import lo valida y lo copia a `<config_dir>/icons/<nombre>/`. Tolerante a archivos
//! corruptos: una entrada inválida no aborta la importación.

use crate::icon_source::IconSource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

/// Error de empaquetado (string simple; se muestra al usuario vía MessageModal).
pub type PackResult<T> = Result<T, String>;

/// Una entrada del manifest: qué objeto y de dónde sale su ícono.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverrideEntry {
    /// Forma string estable de la IconKey (ver icon_source::key_to_string).
    pub key: String,
    pub source: IconSource,
}

/// Manifest de un `.naygoset`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackManifest {
    pub schema: u32,
    pub name: String,
    #[serde(default)]
    pub author: String,
    pub base_set_id: String,
    #[serde(default)]
    pub overrides: Vec<OverrideEntry>,
}

/// Exporta el set efectivo (base + overrides) a un archivo `.naygoset`.
///
/// El archivo resultante es un zip autocontenido con `manifest.json` y todos los
/// PNG de usuario referenciados en `overrides`. Los overrides de tipo `Builtin`
/// solo quedan en el manifest (el PNG lo aporta el set de fábrica al importar).
pub fn export_pack(
    out: &Path,
    name: &str,
    author: &str,
    base_set_id: &str,
    overrides: &BTreeMap<String, IconSource>,
    config_dir: &Path,
) -> PackResult<()> {
    let manifest = PackManifest {
        schema: 1,
        name: name.to_string(),
        author: author.to_string(),
        base_set_id: base_set_id.to_string(),
        overrides: overrides
            .iter()
            .map(|(k, s)| OverrideEntry { key: k.clone(), source: s.clone() })
            .collect(),
    };
    let file =
        std::fs::File::create(out).map_err(|e| format!("crear {}: {e}", out.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    zip.start_file("manifest.json", opts).map_err(|e| e.to_string())?;
    zip.write_all(&json).map_err(|e| e.to_string())?;

    for src in overrides.values() {
        if let IconSource::UserPng { rel_path } = src {
            let p = config_dir.join("icons").join("_user").join(rel_path);
            if let Ok(bytes) = std::fs::read(&p) {
                zip.start_file(format!("icons/_user/{rel_path}"), opts)
                    .map_err(|e| e.to_string())?;
                zip.write_all(&bytes).map_err(|e| e.to_string())?;
            }
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Importa un `.naygoset` a `<config_dir>/icons/<nombre>/`. Devuelve el manifest.
///
/// Tolerante: una entrada PNG corrupta no aborta la importación. Solo el zip
/// inválido o el manifest ilegible son errores fatales.
pub fn import_pack(path: &Path, config_dir: &Path) -> PackResult<PackManifest> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("abrir {}: {e}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip inválido: {e}"))?;

    let manifest: PackManifest = {
        let mf = archive
            .by_name("manifest.json")
            .map_err(|_| "falta manifest.json".to_string())?;
        let mut s = String::new();
        // Cap el manifest a 1 MiB: defensa anti-bomba (un manifest legítimo es minúsculo).
        mf.take(1 << 20).read_to_string(&mut s).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| format!("manifest inválido: {e}"))?
    };
    if manifest.schema != 1 {
        return Err(format!("schema no soportado: {}", manifest.schema));
    }

    // El nombre viene del manifest (controlado por quien creó el .naygoset): sanear.
    let safe_name: String = manifest
        .name
        .chars()
        .map(|c| if c == '/' || c == '\\' || c == ':' { '_' } else { c })
        .collect();
    if safe_name.is_empty() || safe_name == "." || safe_name.contains("..") {
        return Err(format!("nombre de pack inválido: {:?}", manifest.name));
    }
    let dest = config_dir.join("icons").join(&safe_name);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.name().to_string();
        if let Some(rel) = name.strip_prefix("icons/") {
            // Rechazar entradas que escapen del destino (zip-slip): nada con ".." ni rutas absolutas.
            if rel.contains("..") || std::path::Path::new(rel).is_absolute() {
                continue;
            }
            let target = dest.join(rel);
            // Defensa adicional: el destino final debe quedar dentro del dir del pack.
            if !target.starts_with(&dest) {
                continue;
            }
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut buf = Vec::new();
            // Cap cada PNG a 4 MiB: defensa anti-bomba (un ícono real pesa pocos KB).
            if entry.take(4 * 1024 * 1024).read_to_end(&mut buf).is_ok() {
                let _ = std::fs::write(&target, &buf);
            }
        }
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip() {
        let m = PackManifest {
            schema: 1,
            name: "Mi set".into(),
            author: "Nico".into(),
            base_set_id: "lucide".into(),
            overrides: vec![OverrideEntry {
                key: "folder".into(),
                source: IconSource::Builtin { set_id: "material".into() },
            }],
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: PackManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn export_import_round_trip() {
        use std::collections::BTreeMap;
        let cfg = tempfile::tempdir().unwrap();
        let user_dir = cfg.path().join("icons").join("_user");
        std::fs::create_dir_all(&user_dir).unwrap();
        std::fs::write(user_dir.join("ab12.png"), b"\x89PNG\r\n\x1a\nFAKE").unwrap();

        let mut overrides: BTreeMap<String, IconSource> = BTreeMap::new();
        overrides.insert("folder".into(), IconSource::Builtin { set_id: "material".into() });
        overrides.insert("file_image".into(), IconSource::UserPng { rel_path: "ab12.png".into() });

        let out = cfg.path().join("mi.naygoset");
        export_pack(&out, "Mi set", "Nico", "lucide", &overrides, cfg.path()).unwrap();
        assert!(out.exists());

        let cfg2 = tempfile::tempdir().unwrap();
        let imported = import_pack(&out, cfg2.path()).unwrap();
        assert_eq!(imported.base_set_id, "lucide");
        let pack_dir = cfg2.path().join("icons").join(&imported.name);
        assert!(pack_dir.join("_user").join("ab12.png").exists());
    }

    #[test]
    fn import_pack_corrupto_es_err_no_panic() {
        let cfg = tempfile::tempdir().unwrap();
        let bad = cfg.path().join("malo.naygoset");
        std::fs::write(&bad, b"esto no es un zip").unwrap();
        assert!(import_pack(&bad, cfg.path()).is_err());
    }

    #[test]
    fn import_rechaza_entrada_con_traversal() {
        use std::io::Write;
        let cfg = tempfile::tempdir().unwrap();
        let out = cfg.path().join("malo.naygoset");
        let file = std::fs::File::create(&out).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("manifest.json", opts).unwrap();
        zip.write_all(br#"{"schema":1,"name":"x","base_set_id":"lucide"}"#).unwrap();
        zip.start_file("icons/../escape.png", opts).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
        let cfg2 = tempfile::tempdir().unwrap();
        let _ = import_pack(&out, cfg2.path()); // no debe panic
        // el archivo de escape NO debe existir fuera del dir del pack
        assert!(!cfg2.path().join("escape.png").exists());
    }

    #[test]
    fn import_rechaza_nombre_de_pack_con_traversal() {
        use std::io::Write;
        let cfg = tempfile::tempdir().unwrap();
        let out = cfg.path().join("malo2.naygoset");
        let file = std::fs::File::create(&out).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("manifest.json", opts).unwrap();
        zip.write_all(br#"{"schema":1,"name":"../../evil","base_set_id":"lucide"}"#).unwrap();
        zip.finish().unwrap();
        let cfg2 = tempfile::tempdir().unwrap();
        assert!(import_pack(&out, cfg2.path()).is_err());
    }
}
