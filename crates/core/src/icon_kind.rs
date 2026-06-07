// Naygo — clasificación semántica de íconos (lógica pura, sin GPU ni assets).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Mapea un `Entry` (o una extensión) a una **clave de ícono** semántica
//! (`IconKey`), no a un archivo de imagen. La UI traduce la clave al asset del set
//! activo. Puro y testeable: misma extensión → misma clave; sin tocar disco/GPU.

use crate::fs_model::{Entry, EntryKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Categoría semántica de un archivo, derivada de su extensión.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileCategory {
    Image,
    Video,
    Audio,
    Document,
    Code,
    Archive,
    Executable,
    Model3D,
    Font,
    Generic,
}

/// Tipo de unidad de disco. En 2B solo se usa el genérico; la detección real es
/// de `platform` (fase futura).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DriveKind {
    Fixed,
    Removable,
    Network,
    Optical,
    Unknown,
}

/// Clave semántica de un ícono. La UI la traduce al asset del set activo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IconKey {
    /// Carpeta normal.
    Folder,
    /// La fila virtual "..".
    ParentDir,
    /// Archivo de una categoría.
    File(FileCategory),
    /// Unidad de disco.
    Drive(DriveKind),
    /// Fallback genérico (tipo no clasificable).
    Unknown,
}

/// Tabla extensión (minúsculas, sin punto) → categoría. Construida una sola vez.
fn extension_table() -> &'static HashMap<&'static str, FileCategory> {
    static TABLE: OnceLock<HashMap<&'static str, FileCategory>> = OnceLock::new();
    TABLE.get_or_init(|| {
        use FileCategory::*;
        let pairs: &[(&str, FileCategory)] = &[
            // Imagen
            ("png", Image),
            ("jpg", Image),
            ("jpeg", Image),
            ("gif", Image),
            ("bmp", Image),
            ("webp", Image),
            ("tiff", Image),
            ("svg", Image),
            ("ico", Image),
            // Video
            ("mp4", Video),
            ("mkv", Video),
            ("avi", Video),
            ("mov", Video),
            ("webm", Video),
            ("wmv", Video),
            // Audio
            ("mp3", Audio),
            ("wav", Audio),
            ("flac", Audio),
            ("ogg", Audio),
            ("m4a", Audio),
            ("aac", Audio),
            // Documento
            ("pdf", Document),
            ("doc", Document),
            ("docx", Document),
            ("xls", Document),
            ("xlsx", Document),
            ("ppt", Document),
            ("pptx", Document),
            ("odt", Document),
            ("rtf", Document),
            ("md", Document),
            // Código
            ("rs", Code),
            ("py", Code),
            ("js", Code),
            ("ts", Code),
            ("c", Code),
            ("cpp", Code),
            ("h", Code),
            ("java", Code),
            ("go", Code),
            ("rb", Code),
            ("toml", Code),
            ("json", Code),
            ("yaml", Code),
            ("yml", Code),
            ("xml", Code),
            ("html", Code),
            ("css", Code),
            ("sh", Code),
            // Archivo comprimido
            ("zip", Archive),
            ("rar", Archive),
            ("7z", Archive),
            ("tar", Archive),
            ("gz", Archive),
            ("xz", Archive),
            ("bz2", Archive),
            // Ejecutable
            ("exe", Executable),
            ("msi", Executable),
            ("bat", Executable),
            ("cmd", Executable),
            ("com", Executable),
            // Modelo 3D
            ("stl", Model3D),
            ("obj", Model3D),
            ("3mf", Model3D),
            ("step", Model3D),
            ("stp", Model3D),
            ("gcode", Model3D),
            ("fbx", Model3D),
            ("blend", Model3D),
            // Fuente
            ("ttf", Font),
            ("otf", Font),
            ("woff", Font),
            ("woff2", Font),
        ];
        pairs.iter().copied().collect()
    })
}

/// Categoría de una extensión (case-insensitive). Desconocida → `Generic`.
pub fn category_for_extension(ext: &str) -> FileCategory {
    let lower = ext.to_ascii_lowercase();
    extension_table()
        .get(lower.as_str())
        .copied()
        .unwrap_or(FileCategory::Generic)
}

/// Clave de ícono para un `Entry`.
pub fn icon_key_for(entry: &Entry) -> IconKey {
    match entry.kind {
        EntryKind::Directory => IconKey::Folder,
        EntryKind::File => {
            let ext = entry
                .path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            IconKey::File(category_for_extension(ext))
        }
        EntryKind::Other => IconKey::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_model::EntryKind;
    use std::path::PathBuf;

    fn file(path: &str) -> Entry {
        Entry {
            name: path.into(),
            path: PathBuf::from(path),
            kind: EntryKind::File,
            size: Some(1),
            modified: None,
            created: None,
            hidden: false,
        }
    }

    #[test]
    fn extension_a_categoria() {
        assert_eq!(category_for_extension("stl"), FileCategory::Model3D);
        assert_eq!(category_for_extension("jpg"), FileCategory::Image);
        assert_eq!(category_for_extension("zip"), FileCategory::Archive);
        assert_eq!(category_for_extension("exe"), FileCategory::Executable);
        assert_eq!(category_for_extension("rs"), FileCategory::Code);
    }

    #[test]
    fn extension_es_case_insensitive() {
        assert_eq!(category_for_extension("JPG"), FileCategory::Image);
        assert_eq!(category_for_extension("Stl"), FileCategory::Model3D);
    }

    #[test]
    fn extension_desconocida_es_generic() {
        assert_eq!(category_for_extension("xyzabc"), FileCategory::Generic);
        assert_eq!(category_for_extension(""), FileCategory::Generic);
    }

    #[test]
    fn icon_key_de_carpeta_y_archivo() {
        let dir = Entry {
            name: "docs".into(),
            path: PathBuf::from("C:/docs"),
            kind: EntryKind::Directory,
            size: None,
            modified: None,
            created: None,
            hidden: false,
        };
        assert_eq!(icon_key_for(&dir), IconKey::Folder);
        assert_eq!(
            icon_key_for(&file("modelo.stl")),
            IconKey::File(FileCategory::Model3D)
        );
        assert_eq!(
            icon_key_for(&file("sin_extension")),
            IconKey::File(FileCategory::Generic)
        );
    }

    #[test]
    fn icon_key_de_other_es_unknown() {
        let other = Entry {
            name: "raro".into(),
            path: PathBuf::from("C:/raro"),
            kind: EntryKind::Other,
            size: None,
            modified: None,
            created: None,
            hidden: false,
        };
        assert_eq!(icon_key_for(&other), IconKey::Unknown);
    }
}
