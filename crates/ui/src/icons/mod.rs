// Naygo — IconProvider: decodifica y cachea los íconos del set activo en GPU.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Carga los PNG embebidos del set activo UNA vez, los sube a textura egui y los
//! cachea por `IconKey`. Pintar una fila = referenciar una textura ya cargada
//! (cero decodificación por frame). Cambiar de set = `reload` (operación única).

// IconProvider y su API se consumen en Tarea 5 (la lista de archivos pinta filas
// con estas texturas). Hasta entonces, naygo-ui es un binario y `pub` no suprime
// el dead_code; se permite explícitamente para no romper `clippy -D warnings`.
#![allow(dead_code)]

pub mod assets;

use naygo_core::config::IconSet;
use naygo_core::icon_kind::IconKey;
use std::collections::HashMap;

/// Dueño de las texturas del set activo.
pub struct IconProvider {
    set: IconSet,
    textures: HashMap<IconKey, egui::TextureHandle>,
    /// Textura de respaldo (la de `Unknown`), garantizada presente.
    fallback: egui::TextureHandle,
}

impl IconProvider {
    /// Carga el set `set` en el contexto `ctx`.
    pub fn new(ctx: &egui::Context, set: IconSet) -> Self {
        let fallback = load_texture(ctx, set, IconKey::Unknown);
        let mut textures = HashMap::new();
        for key in assets::all_keys() {
            textures.insert(key, load_texture(ctx, set, key));
        }
        IconProvider { set, textures, fallback }
    }

    /// El set actualmente cargado.
    pub fn set(&self) -> IconSet {
        self.set
    }

    /// Recarga el atlas para `set` (operación única, no por-frame).
    pub fn reload(&mut self, ctx: &egui::Context, set: IconSet) {
        if set == self.set {
            return;
        }
        *self = IconProvider::new(ctx, set);
    }

    /// Textura cacheada para `key`; cae al fallback si no está.
    pub fn texture(&self, key: IconKey) -> &egui::TextureHandle {
        self.textures.get(&key).unwrap_or(&self.fallback)
    }
}

/// Decodifica el PNG embebido de `set`+`key` y lo sube como textura. Si el PNG es
/// ilegible, sube una textura 1x1 transparente (nunca crashea).
fn load_texture(ctx: &egui::Context, set: IconSet, key: IconKey) -> egui::TextureHandle {
    let bytes = assets::bytes_for(set, key);
    let color_image = decode_png(bytes).unwrap_or_else(|| {
        tracing::warn!("ícono ilegible para {:?}/{:?}; usando vacío", set, key);
        egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0])
    });
    let name = format!("icon_{:?}_{:?}", set, key);
    ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
}

/// Decodifica bytes PNG a `ColorImage` RGBA, o `None` si falla.
fn decode_png(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}
