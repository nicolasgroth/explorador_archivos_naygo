// Naygo — cache de íconos: PNG embebido → slint::Image, decodificado UNA sola vez. Clave del
// rendimiento: el render por software no re-rasteriza; clonar un slint::Image comparte el buffer.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
// El IconCache se cablea por partes en las tareas 3 y 4 de 6A (filas, combo de set). Hasta
// entonces, algunos métodos quedan sin usar; el allow evita el ruido de clippy mientras tanto.
#![allow(dead_code)]
use naygo_core::config::IconSet;
use naygo_core::icon_kind::IconKey;
use slint::{Image, SharedPixelBuffer};
use std::collections::HashMap;

/// Decodifica y cachea los íconos a `slint::Image` por (set, clave). El set activo lo fija la UI.
pub struct IconCache {
    map: HashMap<(IconSet, IconKey), Image>,
    /// Set activo (el que devuelve `get`). La UI lo cambia con `set_active`.
    active: IconSet,
}

impl IconCache {
    pub fn new(active: IconSet) -> IconCache {
        IconCache {
            map: HashMap::new(),
            active,
        }
    }

    /// Cambia el set activo (al cambiarlo en Configuración). No borra el cache: las claves del
    /// set nuevo se decodifican on-demand; las viejas quedan (memoria despreciable, 28 PNGs).
    // `set_active`/`active` se cablean en las tareas siguientes (combo de set de íconos).
    #[allow(dead_code)]
    pub fn set_active(&mut self, set: IconSet) {
        self.active = set;
    }

    #[allow(dead_code)]
    pub fn active(&self) -> IconSet {
        self.active
    }

    /// El `slint::Image` del ícono `key` en el set activo. Lo decodifica si falta y lo cachea.
    /// Si el PNG es ilegible, devuelve una imagen vacía (no crashea).
    pub fn get(&mut self, key: IconKey) -> Image {
        let set = self.active;
        if let Some(img) = self.map.get(&(set, key)) {
            return img.clone();
        }
        let img = decode(naygo_core::icons::bytes_for(set, key));
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

    #[test]
    fn get_cachea_la_misma_clave() {
        let mut c = IconCache::new(IconSet::Flat);
        let a = c.get(IconKey::Folder);
        let b = c.get(IconKey::Folder);
        // El size es estable y no vacío (el PNG de carpeta existe).
        assert_eq!(a.size(), b.size());
        assert!(a.size().width > 0);
    }
}
