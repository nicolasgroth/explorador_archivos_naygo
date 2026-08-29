// Naygo — carpetas conocidas de Windows y proveedores de nube instalados.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Resuelve accesos especiales a rutas físicas. La UI nunca navega un PIDL ni una ruta virtual:
//! cada resultado es un `PathBuf` real que puede listar el worker normal de Naygo.

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownFolder {
    pub name: String,
    pub path: PathBuf,
    /// Clave visual estable para el árbol. La ruta sigue siendo física; este dato solo evita
    /// que Escritorio, Documentos o proveedores cloud parezcan carpetas genéricas.
    pub icon: KnownFolderIcon,
}

/// Familias de accesos especiales que Windows resuelve a carpetas físicas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownFolderIcon {
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Cloud,
}

impl KnownFolderIcon {
    pub const fn tree_tag(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Documents => "documents",
            Self::Downloads => "downloads",
            Self::Pictures => "pictures",
            Self::Music => "music",
            Self::Videos => "videos",
            Self::Cloud => "cloud",
        }
    }
}

#[cfg(windows)]
pub fn known_folders() -> Vec<KnownFolder> {
    static CACHE: std::sync::OnceLock<Vec<KnownFolder>> = std::sync::OnceLock::new();
    CACHE.get_or_init(discover_known_folders).clone()
}

#[cfg(windows)]
fn discover_known_folders() -> Vec<KnownFolder> {
    use windows::Win32::UI::Shell::{
        FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music,
        FOLDERID_Pictures, FOLDERID_SkyDrive, FOLDERID_Videos,
    };

    let mut result = Vec::new();
    for (id, icon) in [
        (FOLDERID_Desktop, KnownFolderIcon::Desktop),
        (FOLDERID_Documents, KnownFolderIcon::Documents),
        (FOLDERID_Downloads, KnownFolderIcon::Downloads),
        (FOLDERID_Pictures, KnownFolderIcon::Pictures),
        (FOLDERID_Music, KnownFolderIcon::Music),
        (FOLDERID_Videos, KnownFolderIcon::Videos),
        (FOLDERID_SkyDrive, KnownFolderIcon::Cloud),
    ] {
        if let Some(path) = path_for(&id) {
            push_unique(&mut result, path, icon);
        }
    }

    // Algunas instalaciones empresariales de OneDrive no registran FOLDERID_SkyDrive o
    // mantienen más de una cuenta. Las variables oficiales contienen las rutas físicas.
    for key in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        if let Some(path) = std::env::var_os(key).map(PathBuf::from) {
            push_unique(&mut result, path, KnownFolderIcon::Cloud);
        }
    }

    // Dropbox publica la ruta de cada cuenta en info.json. Esto cubre ubicaciones reubicadas
    // como G:\Dropbox\Dropbox, que no se pueden inferir desde USERPROFILE.
    for base in ["LOCALAPPDATA", "APPDATA"] {
        let Some(root) = std::env::var_os(base).map(PathBuf::from) else {
            continue;
        };
        let info = root.join("Dropbox").join("info.json");
        let Ok(bytes) = std::fs::read(info) else {
            continue;
        };
        let Ok(accounts) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        let Some(accounts) = accounts.as_object() else {
            continue;
        };
        for account in accounts.values() {
            if let Some(path) = account.get("path").and_then(|value| value.as_str()) {
                // Dropbox es una carpeta física, pero se identifica como acceso cloud al igual
                // que OneDrive. El `push_unique` mantiene la primera etiqueta si coincide.
                push_unique(&mut result, PathBuf::from(path), KnownFolderIcon::Cloud);
            }
        }
    }
    result
}

#[cfg(windows)]
fn path_for(id: &windows::core::GUID) -> Option<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};

    // SAFETY: SHGetKnownFolderPath asigna el PWSTR con CoTaskMemAlloc. Copiamos el contenido a
    // String antes de liberarlo exactamente una vez con CoTaskMemFree.
    unsafe {
        let raw = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let text = raw.to_string().ok();
        CoTaskMemFree(Some(raw.0.cast()));
        text.filter(|value| !value.is_empty()).map(PathBuf::from)
    }
}

#[cfg(windows)]
fn display_name(path: &std::path::Path) -> String {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_DISPLAYNAME};

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut info = SHFILEINFOW::default();
    // SAFETY: `wide` y `info` viven durante la llamada; cbFileInfo coincide con la estructura.
    let ok = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_DISPLAYNAME,
        )
    } != 0;
    if ok {
        let end = info
            .szDisplayName
            .iter()
            .position(|ch| *ch == 0)
            .unwrap_or(info.szDisplayName.len());
        let name = String::from_utf16_lossy(&info.szDisplayName[..end]);
        if !name.is_empty() {
            return name;
        }
    }
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(windows)]
fn push_unique(result: &mut Vec<KnownFolder>, path: PathBuf, icon: KnownFolderIcon) {
    if !path.is_dir() {
        return;
    }
    let key = path.to_string_lossy().replace('/', "\\").to_lowercase();
    if result.iter().any(|item| {
        item.path
            .to_string_lossy()
            .replace('/', "\\")
            .to_lowercase()
            == key
    }) {
        return;
    }
    result.push(KnownFolder {
        name: display_name(&path),
        path,
        icon,
    });
}

#[cfg(not(windows))]
pub fn known_folders() -> Vec<KnownFolder> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn carpetas_conocidas_son_fisicas_y_no_se_repitien() {
        let folders = known_folders();
        assert!(!folders.is_empty());
        assert!(folders.iter().all(|folder| folder.path.is_absolute()));
        let mut keys: Vec<String> = folders
            .iter()
            .map(|folder| folder.path.to_string_lossy().to_lowercase())
            .collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), folders.len());
    }
}
