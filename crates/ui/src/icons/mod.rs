// Naygo — IconProvider: decodifica y cachea los íconos del set activo en GPU.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Carga los PNG embebidos del set activo UNA vez, los sube a textura egui y los
//! cachea por `IconKey`. Pintar una fila = referenciar una textura ya cargada
//! (cero decodificación por frame). Cambiar de set = `reload` (operación única).

pub mod assets;

use naygo_core::icon_kind::IconKey;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Dueño de las texturas del set activo.
pub struct IconProvider {
    set_id: String,
    textures: HashMap<IconKey, egui::TextureHandle>,
    /// Textura de respaldo (la de `Unknown`), garantizada presente.
    fallback: egui::TextureHandle,
}

/// Mapea un id de set embebido a su enum; ids desconocidos → Flat (solo se consulta
/// para ids de sets embebidos; los packs sueltos resuelven desde disco).
fn embedded_set(id: &str) -> naygo_core::config::IconSet {
    use naygo_core::config::IconSet;
    match id {
        "fluent" => IconSet::Fluent,
        "mono" => IconSet::Mono,
        _ => IconSet::Flat,
    }
}

impl IconProvider {
    /// Carga el set con id `set_id` (embebido o pack suelto bajo `config_dir`).
    pub fn new(ctx: &egui::Context, set_id: &str, config_dir: &Path) -> Self {
        let loader = TextureLoader {
            set_id: set_id.to_string(),
            config_dir: config_dir.to_path_buf(),
        };
        let fallback = loader.load(ctx, IconKey::Unknown);
        let mut textures = HashMap::new();
        for key in assets::all_keys() {
            textures.insert(key, loader.load(ctx, key));
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
    pub fn reload(&mut self, ctx: &egui::Context, set_id: &str, config_dir: &Path) {
        if set_id == self.set_id {
            return;
        }
        *self = IconProvider::new(ctx, set_id, config_dir);
    }

    /// Textura cacheada para `key`; cae al fallback si no está.
    pub fn texture(&self, key: IconKey) -> &egui::TextureHandle {
        self.textures.get(&key).unwrap_or(&self.fallback)
    }
}

/// Resuelve y sube a textura el ícono de una `key` para un set (embebido o suelto).
struct TextureLoader {
    set_id: String,
    config_dir: PathBuf,
}

impl TextureLoader {
    /// Sube la textura del ícono de `key`. Si nada resuelve, sube una 1x1 transparente
    /// (nunca crashea).
    fn load(&self, ctx: &egui::Context, key: IconKey) -> egui::TextureHandle {
        let color_image = self.color_image_for(key).unwrap_or_else(|| {
            tracing::warn!(
                "ícono ilegible para {}/{:?}; usando vacío",
                self.set_id,
                key
            );
            egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0])
        });
        let name = format!("icon_{}_{:?}", self.set_id, key);
        ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
    }

    /// Decodifica la imagen del ícono: de los assets embebidos para los sets
    /// `flat`/`fluent`/`mono`, o de `<config>/icons/<id>/<file_name>.png` para un pack
    /// suelto, con fallback al `unknown` embebido (flat) si el archivo falta o es ilegible.
    fn color_image_for(&self, key: IconKey) -> Option<egui::ColorImage> {
        let builtin = matches!(self.set_id.as_str(), "flat" | "fluent" | "mono");
        if builtin {
            decode_png(assets::bytes_for(embedded_set(&self.set_id), key))
        } else {
            let path = self
                .config_dir
                .join("icons")
                .join(&self.set_id)
                .join(format!("{}.png", assets::file_name(key)));
            let from_disk = std::fs::read(&path).ok().and_then(|b| decode_png(&b));
            from_disk.or_else(|| {
                decode_png(assets::bytes_for(
                    naygo_core::config::IconSet::Flat,
                    IconKey::Unknown,
                ))
            })
        }
    }
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
