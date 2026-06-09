// Naygo — IconProvider: decodifica y cachea los íconos del set activo en GPU.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Carga los PNG embebidos del set activo UNA vez, los sube a textura egui y los
//! cachea por `IconKey`. Pintar una fila = referenciar una textura ya cargada
//! (cero decodificación por frame). Cambiar de set = `reload` (operación única).

pub mod assets;

use naygo_core::icon_kind::IconKey;
use std::collections::HashMap;

/// Dueño de las texturas del set activo.
pub struct IconProvider {
    set_id: String,
    textures: HashMap<IconKey, egui::TextureHandle>,
    /// Textura de respaldo (la de `Unknown`), garantizada presente.
    fallback: egui::TextureHandle,
}

/// Mapea un id de set embebido a su enum; ids desconocidos → Flat (fallback temporal,
/// los packs sueltos llegan en una tarea posterior).
fn embedded_set(id: &str) -> naygo_core::config::IconSet {
    use naygo_core::config::IconSet;
    match id {
        "fluent" => IconSet::Fluent,
        "mono" => IconSet::Mono,
        _ => IconSet::Flat,
    }
}

impl IconProvider {
    /// Carga el set con id `set_id` en el contexto `ctx`.
    pub fn new(ctx: &egui::Context, set_id: &str) -> Self {
        let fallback = load_texture(ctx, set_id, IconKey::Unknown);
        let mut textures = HashMap::new();
        for key in assets::all_keys() {
            textures.insert(key, load_texture(ctx, set_id, key));
        }
        IconProvider {
            set_id: set_id.to_string(),
            textures,
            fallback,
        }
    }

    /// El id del set actualmente cargado.
    pub fn set(&self) -> &str {
        &self.set_id
    }

    /// Recarga el atlas para `set_id` (operación única, no por-frame).
    pub fn reload(&mut self, ctx: &egui::Context, set_id: &str) {
        if set_id == self.set_id {
            return;
        }
        *self = IconProvider::new(ctx, set_id);
    }

    /// Textura cacheada para `key`; cae al fallback si no está.
    pub fn texture(&self, key: IconKey) -> &egui::TextureHandle {
        self.textures.get(&key).unwrap_or(&self.fallback)
    }
}

/// Decodifica el PNG embebido de `set_id`+`key` y lo sube como textura. Si el PNG es
/// ilegible, sube una textura 1x1 transparente (nunca crashea).
fn load_texture(ctx: &egui::Context, set_id: &str, key: IconKey) -> egui::TextureHandle {
    let set = embedded_set(set_id);
    let bytes = assets::bytes_for(set, key);
    let color_image = decode_png(bytes).unwrap_or_else(|| {
        tracing::warn!("ícono ilegible para {set_id}/{:?}; usando vacío", key);
        egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0])
    });
    let name = format!("icon_{set_id}_{:?}", key);
    ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
}

/// Decodifica bytes PNG a `ColorImage` RGBA, o `None` si falla.
fn decode_png(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        rgba.as_raw(),
    ))
}
