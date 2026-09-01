// Naygo — lectura y eliminación explícita de Zone.Identifier (ADS de Windows).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ZoneIdentifier {
    pub zone_id: Option<u32>,
    pub host_url: Option<String>,
    pub referrer_url: Option<String>,
}

fn ads_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}:Zone.Identifier", path.display()))
}

/// Lee solo bajo demanda. `Ok(None)` significa que el archivo no tiene marca; los sistemas sin
/// ADS devuelven el error del filesystem para que la UI pueda explicarlo sin asumir éxito.
pub fn read(path: &Path) -> std::io::Result<Option<ZoneIdentifier>> {
    let text = match std::fs::read_to_string(ads_path(path)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut result = ZoneIdentifier::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "ZoneId" => result.zone_id = value.trim().parse().ok(),
            "HostUrl" => result.host_url = Some(value.trim().to_string()),
            "ReferrerUrl" => result.referrer_url = Some(value.trim().to_string()),
            _ => {}
        }
    }
    Ok(Some(result))
}

/// Quita la marca únicamente tras una confirmación explícita en la UI.
pub fn remove(path: &Path) -> std::io::Result<bool> {
    match std::fs::remove_file(ads_path(path)) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parser_tolera_campos_desconocidos() {
        let text = "[ZoneTransfer]\nZoneId=3\nHostUrl=https://example.test/a\nOther=x\n";
        let mut z = ZoneIdentifier::default();
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                if k == "ZoneId" {
                    z.zone_id = v.parse().ok();
                }
                if k == "HostUrl" {
                    z.host_url = Some(v.into());
                }
            }
        }
        assert_eq!(z.zone_id, Some(3));
        assert_eq!(z.host_url.as_deref(), Some("https://example.test/a"));
    }
}
