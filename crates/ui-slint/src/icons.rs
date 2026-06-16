// Naygo — cache de íconos: PNG (embebido o pack suelto del usuario) → slint::Image, decodificado
// UNA sola vez. Clave del rendimiento: el render por software no re-rasteriza; clonar un
// slint::Image comparte el buffer. El set activo es un id (String), igual que en la capa egui:
// `flat`/`fluent`/`mono` resuelven a los assets embebidos; cualquier otro id es un pack suelto
// del usuario bajo `<config_dir>/icons/<id>/<name>.png` (6E). Ver `naygo_core::icons`.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
// El IconCache se cablea por partes en las tareas 3 y 4 de 6A (filas, combo de set). Hasta
// entonces, algunos métodos quedan sin usar; el allow evita el ruido de clippy mientras tanto.
#![allow(dead_code)]
use naygo_core::icon_kind::IconKey;
use slint::{Image, SharedPixelBuffer};
use std::collections::HashMap;
use std::path::PathBuf;

/// Decodifica y cachea los íconos a `slint::Image` por (set_id, clave). El set activo lo fija
/// la configuración (`Settings.icon_set`, un id de set). El `config_dir` permite resolver los
/// packs sueltos del usuario desde disco.
pub struct IconCache {
    map: HashMap<(String, IconKey), Image>,
    /// Id del set activo (el que devuelve `get`). La UI lo cambia con `set_active`.
    active: String,
    /// Directorio de configuración portable: contiene `icons/<id>/` de los packs del usuario.
    config_dir: PathBuf,
}

impl IconCache {
    pub fn new(active: impl Into<String>, config_dir: PathBuf) -> IconCache {
        IconCache {
            map: HashMap::new(),
            active: active.into(),
            config_dir,
        }
    }

    /// Cambia el set activo (al cambiarlo en Configuración). No borra el cache: las claves del
    /// set nuevo se decodifican on-demand; las viejas quedan (memoria despreciable, 28 PNGs).
    pub fn set_active(&mut self, set_id: impl Into<String>) {
        self.active = set_id.into();
    }

    pub fn active(&self) -> &str {
        &self.active
    }

    /// El `slint::Image` del ícono `key` en el set activo. Lo decodifica si falta y lo cachea.
    /// Si el PNG es ilegible, devuelve una imagen vacía (no crashea).
    pub fn get(&mut self, key: IconKey) -> Image {
        let set = self.active.clone();
        if let Some(img) = self.map.get(&(set.clone(), key)) {
            return img.clone();
        }
        let bytes = naygo_core::icons::resolve_bytes(&set, key, &self.config_dir);
        let img = decode(&bytes);
        self.map.insert((set, key), img.clone());
        img
    }
}

/// Decodifica bytes PNG a un `slint::Image` RGBA. Imagen vacía si falla.
fn decode(bytes: &[u8]) -> Image {
    match image::load_from_memory_with_format(bytes, image::ImageFormat::Png) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            let buf = SharedPixelBuffer::clone_from_slice(rgba.as_raw(), w, h);
            Image::from_rgba8(buf)
        }
        Err(_) => Image::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::icon_kind::IconKey;

    fn cache() -> IconCache {
        IconCache::new("flat", std::path::PathBuf::new())
    }

    #[test]
    fn get_cachea_la_misma_clave() {
        let mut c = cache();
        let a = c.get(IconKey::Folder);
        let b = c.get(IconKey::Folder);
        // El size es estable y no vacío (el PNG de carpeta existe).
        assert_eq!(a.size(), b.size());
        assert!(a.size().width > 0);
    }
}
