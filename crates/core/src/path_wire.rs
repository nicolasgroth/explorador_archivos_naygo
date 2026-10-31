// Naygo — codificación JSON reversible de rutas nativas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Serialización compatible de rutas: los paths UTF-8 siguen siendo strings; en Unix,
//! los bytes no UTF-8 se guardan como `{encoding:"unix-bytes-v1",bytes_hex:"…"}`.
//! Nunca se reconstruye una ruta operativa desde su texto de presentación.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedPath {
    encoding: String,
    bytes_hex: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PathWire {
    Utf8(String),
    Encoded(EncodedPath),
}

struct PathRef<'a>(&'a Path);

impl Serialize for PathRef<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(value) = self.0.to_str() {
            return serializer.serialize_str(value);
        }

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;

            let bytes_hex = self
                .0
                .as_os_str()
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            EncodedPath {
                encoding: "unix-bytes-v1".to_owned(),
                bytes_hex,
            }
            .serialize(serializer)
        }

        #[cfg(not(unix))]
        {
            Err(serde::ser::Error::custom(
                "la ruta contiene unidades nativas que este formato no admite",
            ))
        }
    }
}

struct OwnedPath(PathBuf);

impl<'de> Deserialize<'de> for OwnedPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match PathWire::deserialize(deserializer)? {
            PathWire::Utf8(value) => Ok(Self(PathBuf::from(value))),
            PathWire::Encoded(value) => {
                if value.encoding != "unix-bytes-v1" {
                    return Err(serde::de::Error::custom("codificación de ruta desconocida"));
                }
                let bytes = decode_hex(&value.bytes_hex).ok_or_else(|| {
                    serde::de::Error::custom("bytes_hex no es hexadecimal válido")
                })?;

                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStringExt;
                    Ok(Self(PathBuf::from(std::ffi::OsString::from_vec(bytes))))
                }

                #[cfg(not(unix))]
                {
                    let _ = bytes;
                    Err(serde::de::Error::custom(
                        "una ruta POSIX codificada solo se puede restaurar en Unix",
                    ))
                }
            }
        }
    }
}

/// `serde(with = "crate::path_wire")` para un `PathBuf`.
pub fn serialize<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    PathRef(path).serialize(serializer)
}

/// `serde(with = "crate::path_wire")` para un `PathBuf`.
pub fn deserialize<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    OwnedPath::deserialize(deserializer).map(|path| path.0)
}

/// Adaptador para `Option<PathBuf>`.
pub mod option {
    use super::{OwnedPath, PathRef};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::path::PathBuf;

    pub fn serialize<S>(path: &Option<PathBuf>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        path.as_deref().map(PathRef).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<OwnedPath>::deserialize(deserializer).map(|path| path.map(|path| path.0))
    }
}

/// Adaptador para `Vec<PathBuf>`.
pub mod vec {
    use super::{OwnedPath, PathRef};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::path::PathBuf;

    pub fn serialize<S>(paths: &[PathBuf], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        paths
            .iter()
            .map(|path| PathRef(path))
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<PathBuf>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<OwnedPath>::deserialize(deserializer)
            .map(|paths| paths.into_iter().map(|path| path.0).collect())
    }
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    let source = value.as_bytes();
    if !source.len().is_multiple_of(2) {
        return None;
    }

    source
        .chunks_exact(2)
        .map(|pair| Some((nibble(pair[0])? << 4) | nibble(pair[1])?))
        .collect()
}

fn nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Paths {
        #[serde(with = "crate::path_wire")]
        one: PathBuf,
        #[serde(with = "crate::path_wire::option")]
        optional: Option<PathBuf>,
        #[serde(with = "crate::path_wire::vec")]
        many: Vec<PathBuf>,
    }

    #[test]
    fn utf8_paths_keep_the_existing_json_string_shape() {
        let value = Paths {
            one: PathBuf::from("/home/nico/Documentos"),
            optional: None,
            many: vec![PathBuf::from("/tmp/archivo ñ.txt")],
        };
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(
            json,
            r#"{"one":"/home/nico/Documentos","optional":null,"many":["/tmp/archivo ñ.txt"]}"#
        );
        assert_eq!(serde_json::from_str::<Paths>(&json).unwrap(), value);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_round_trip_as_tagged_bytes_in_all_adapters() {
        use std::os::unix::ffi::OsStringExt;

        let invalid = PathBuf::from(std::ffi::OsString::from_vec(
            b"/tmp/dir-\xff/file-\xfe".to_vec(),
        ));
        let value = Paths {
            one: invalid.clone(),
            optional: Some(invalid.clone()),
            many: vec![PathBuf::from("/tmp/ok"), invalid.clone()],
        };
        let json = serde_json::to_string(&value).unwrap();
        assert!(json.contains(r#""encoding":"unix-bytes-v1""#));
        assert!(json.contains("2f746d702f6469722dff2f66696c652dfe"));
        assert_eq!(serde_json::from_str::<Paths>(&json).unwrap(), value);
    }

    #[test]
    fn malformed_or_unknown_tagged_path_is_rejected() {
        assert!(serde_json::from_str::<OwnedPath>(
            r#"{"encoding":"future-v2","bytes_hex":"2f746d70"}"#
        )
        .is_err());
        assert!(serde_json::from_str::<OwnedPath>(
            r#"{"encoding":"unix-bytes-v1","bytes_hex":"not-hex"}"#
        )
        .is_err());
        assert!(serde_json::from_str::<OwnedPath>(
            r#"{"encoding":"unix-bytes-v1","bytes_hex":"2f","extra":true}"#
        )
        .is_err());
    }
}
