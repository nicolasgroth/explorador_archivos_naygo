// Naygo — cache de íconos: PNG (embebido o pack suelto del usuario) → slint::Image, decodificado
// UNA sola vez. Clave del rendimiento: el render por software no re-rasteriza; clonar un
// slint::Image comparte el buffer. El set activo es un id (String), igual que en la capa egui:
// `flat`/`fluent`/`mono` resuelven a los assets embebidos; cualquier otro id es un pack suelto
// del usuario bajo `<config_dir>/icons/<id>/<name>.png` (6E). Ver `naygo_core::icons`.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
// El IconCache se cablea por partes en las tareas 3 y 4 de 6A (filas, combo de set). Hasta
// entonces, algunos métodos quedan sin usar; el allow evita el ruido de clippy mientras tanto.
#![allow(dead_code)]
use naygo_core::icon_kind::IconKey;
use slint::{Image, SharedPixelBuffer};
use std::collections::HashMap;
use std::path::PathBuf;

/// Clave del cache: (set_activo, ícono, color_tinte, tintable).
// La clave NO incluye el set activo: `set_active` limpia el cache al cambiar de set, así el map
// solo contiene íconos del set vigente. Evita clonar el `String` del set en cada `get` (camino
// caliente, una vez por fila), a cambio de re-decodificar al cambiar de set (acción rara).
type CacheKey = (IconKey, (u8, u8, u8), bool);

/// Decodifica y cachea los íconos a `slint::Image` por (set_id, clave). El set activo lo fija
/// la configuración (`Settings.icon_set`, un id de set). El `config_dir` permite resolver los
/// packs sueltos del usuario desde disco.
pub struct IconCache {
    /// Clave: (set_activo, clave_ícono, color_tinte, tintable). Incluye el tinte para
    /// que al cambiar el color del tema se fuerce re-decodificación sin limpiar el cache.
    map: HashMap<CacheKey, Image>,
    /// Id del set activo (el que devuelve `get`). La UI lo cambia con `set_active`.
    active: String,
    /// Directorio de configuración portable: contiene `icons/<id>/` de los packs del usuario.
    config_dir: PathBuf,
    /// Overrides por nombre de archivo de ícono (p.ej. "folder") → fuente alternativa.
    overrides: std::collections::BTreeMap<String, naygo_core::icon_source::IconSource>,
    /// Color de tinte (RGB) cuando el set activo es tintable.
    tint: (u8, u8, u8),
    /// Si es `true`, el set activo es una máscara blanca que se tiñe con `tint`.
    tintable: bool,
}

impl IconCache {
    pub fn new(active: impl Into<String>, config_dir: PathBuf) -> IconCache {
        IconCache {
            map: HashMap::new(),
            active: active.into(),
            config_dir,
            overrides: std::collections::BTreeMap::new(),
            tint: (0, 0, 0),
            tintable: false,
        }
    }

    /// Reemplaza los overrides de ícono (mapa nombre → fuente). Se llama al cargar la config
    /// y cada vez que el usuario cambia un override. Invalida el cache completo: las entradas
    /// anteriores podrían haberse resuelto sin el override nuevo.
    pub fn set_overrides(
        &mut self,
        ov: std::collections::BTreeMap<String, naygo_core::icon_source::IconSource>,
    ) {
        self.overrides = ov;
        self.map.clear();
    }

    /// Configura el tinte del set activo. `tintable` indica si el set es máscara blanca;
    /// `rgb` es el color del tema. Si `tintable` es `false`, no se aplica ningún tinte.
    /// Invalida el cache porque el color de tinte está embebido en los píxeles cacheados.
    pub fn set_tint(&mut self, tintable: bool, rgb: (u8, u8, u8)) {
        self.tintable = tintable;
        self.tint = rgb;
        self.map.clear();
    }

    /// Solo para tests: número de entradas actualmente en el cache.
    #[cfg(test)]
    pub fn cache_len(&self) -> usize {
        self.map.len()
    }

    /// Cambia el set activo (al cambiarlo en Configuración). No borra el cache: las claves del
    /// set nuevo se decodifican on-demand; las viejas quedan (memoria despreciable, 28 PNGs).
    pub fn set_active(&mut self, set_id: impl Into<String>) {
        let new = set_id.into();
        if new != self.active {
            // El cache es solo del set vigente (la clave ya no lleva el set): al cambiar, vaciarlo.
            self.map.clear();
            self.active = new;
        }
    }

    pub fn active(&self) -> &str {
        &self.active
    }

    /// Firma O(k) (k = nº de overrides, normalmente 0) del estado que determina el `slint::Image`
    /// de CADA fila: set activo + tinte + tintable + overrides. Si esta firma no cambia, todos los
    /// íconos que `get` devolvería son idénticos (mismo buffer cacheado). La usa `rows_signature`
    /// para incluir el ícono en la firma por panel sin re-decodificar nada. NO aloca: hashea por
    /// referencia los bytes del id/overrides.
    pub fn signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.active.hash(&mut h);
        self.tint.hash(&mut h);
        self.tintable.hash(&mut h);
        // El BTreeMap recorre ordenado → hash estable. Normalmente vacío (sin overrides).
        for (name, src) in &self.overrides {
            name.hash(&mut h);
            // `IconSource` no deriva Hash en core: hashear sus variantes a mano (un id de set o
            // una ruta relativa). El discriminante separa Builtin de UserPng con el mismo string.
            match src {
                naygo_core::icon_source::IconSource::Builtin { set_id } => {
                    0u8.hash(&mut h);
                    set_id.hash(&mut h);
                }
                naygo_core::icon_source::IconSource::UserPng { rel_path } => {
                    1u8.hash(&mut h);
                    rel_path.hash(&mut h);
                }
            }
        }
        h.finish()
    }

    /// El `slint::Image` del ícono `key` en el set activo, aplicando overrides y tinte.
    /// Lo decodifica si falta y lo cachea. Si el PNG es ilegible, devuelve imagen vacía.
    pub fn get(&mut self, key: IconKey) -> Image {
        let tint = if self.tintable { self.tint } else { (0, 0, 0) };
        let ck = (key, tint, self.tintable);
        if let Some(img) = self.map.get(&ck) {
            return img.clone();
        }
        let bytes = naygo_core::icons::resolve_with_overrides(
            &self.active,
            &self.overrides,
            key,
            &self.config_dir,
        );
        let bytes = if self.tintable {
            tint_png(&bytes, self.tint)
        } else {
            bytes
        };
        let img = decode(&bytes);
        self.map.insert(ck, img.clone());
        img
    }
}

/// Renderiza el `key` en un set específico (no el activo), aplicando tinte si corresponde.
/// Se usa en el picker de ícono para mostrar el mismo objeto en cada set disponible.
pub(crate) fn render_for_set(
    key: naygo_core::icon_kind::IconKey,
    set_id: &str,
    tintable: bool,
    tint: (u8, u8, u8),
    config_dir: &std::path::Path,
) -> Image {
    let empty_ov = std::collections::BTreeMap::new();
    let bytes = naygo_core::icons::resolve_with_overrides(set_id, &empty_ov, key, config_dir);
    let bytes = if tintable {
        tint_png(&bytes, tint)
    } else {
        bytes
    };
    decode(&bytes)
}

/// Decodifica bytes PNG a un `slint::Image` RGBA. Imagen vacía si falla.
pub(crate) fn decode(bytes: &[u8]) -> Image {
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

/// Recolorea un PNG (máscara) al color `(r,g,b)` conservando su canal alfa.
/// Para sets tintables: el glifo blanco se vuelve del color del tema.
pub fn tint_png(bytes: &[u8], rgb: (u8, u8, u8)) -> Vec<u8> {
    let decoded = match image::load_from_memory(bytes) {
        Ok(d) => d.to_rgba8(),
        Err(_) => return bytes.to_vec(),
    };
    let (w, h) = (decoded.width(), decoded.height());
    let mut out = image::RgbaImage::new(w, h);
    for (x, y, p) in decoded.enumerate_pixels() {
        out.put_pixel(x, y, image::Rgba([rgb.0, rgb.1, rgb.2, p[3]]));
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    if image::DynamicImage::ImageRgba8(out)
        .write_to(&mut buf, image::ImageFormat::Png)
        .is_err()
    {
        return bytes.to_vec();
    }
    buf.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::icon_kind::IconKey;

    fn cache() -> IconCache {
        IconCache::new("lucide", std::path::PathBuf::new())
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

    #[test]
    fn cache_aplica_override_y_tiñe() {
        use naygo_core::icon_source::IconSource;
        use std::collections::BTreeMap;
        let mut ov: BTreeMap<String, IconSource> = BTreeMap::new();
        ov.insert(
            "folder".into(),
            IconSource::Builtin {
                set_id: "material".into(),
            },
        );
        let mut c = IconCache::new("lucide", std::path::PathBuf::new());
        c.set_overrides(ov);
        c.set_tint(true, (200, 100, 50)); // set tintable: aplica color
        let img = c.get(IconKey::Folder);
        assert!(img.size().width > 0); // se resolvió y decodificó sin panic
    }

    #[test]
    fn set_overrides_invalida_el_cache() {
        use naygo_core::icon_source::IconSource;
        use std::collections::BTreeMap;
        let mut c = IconCache::new("lucide", std::path::PathBuf::new());
        // Cachear folder del set base (lucide).
        let _ = c.get(IconKey::Folder);
        assert_eq!(
            c.cache_len(),
            1,
            "debe haber 1 entrada cacheada tras el primer get"
        );
        // Aplicar override: folder pasa a resolverse desde "material".
        let mut ov: BTreeMap<String, IconSource> = BTreeMap::new();
        ov.insert(
            "folder".into(),
            IconSource::Builtin {
                set_id: "material".into(),
            },
        );
        c.set_overrides(ov);
        assert_eq!(
            c.cache_len(),
            0,
            "set_overrides debe invalidar el cache completo"
        );
        // Re-pedir: debe re-decodificar con el override aplicado.
        let _ = c.get(IconKey::Folder);
        assert_eq!(
            c.cache_len(),
            1,
            "se re-decodifica con el override aplicado"
        );
    }

    #[test]
    fn tint_recolorea_conservando_alfa() {
        // un PNG blanco semitransparente: tras teñir a rojo, el RGB es rojo y el alfa se conserva.
        let mut buf = image::RgbaImage::new(1, 1);
        buf.put_pixel(0, 0, image::Rgba([255, 255, 255, 128]));
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(buf)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let out = tint_png(png.get_ref(), (255, 0, 0));
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        let px = img.get_pixel(0, 0);
        assert_eq!(px[0], 255);
        assert_eq!(px[1], 0);
        assert_eq!(px[2], 0);
        assert_eq!(px[3], 128, "el alfa se conserva");
    }
}
