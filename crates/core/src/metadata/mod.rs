// Naygo — sistema de metadata por tipo (trait + registro + proveedores).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Metadata extra por tipo de archivo, extensible: cada `MetadataProvider` sabe leer los
//! campos de un tipo (imagen, exe, …). El registro mapea extensión → proveedor. Puro salvo
//! los proveedores que toquen el SO (esos se inyectan desde `platform`). Tolerante: un
//! proveedor que falla devuelve vacío; nunca panic (el filesystem es hostil).

use std::path::Path;

pub mod audio_meta;
pub mod image_meta;
pub mod office_meta;

/// Un campo de metadata: la clave i18n de su etiqueta + el valor ya formateado.
#[derive(Clone, Debug, PartialEq)]
pub struct MetadataField {
    pub label_key: &'static str,
    pub value: String,
}

/// Un proveedor de metadata para uno o más tipos de archivo.
pub trait MetadataProvider: Send + Sync {
    /// Extensiones (lowercase, sin punto) que este proveedor maneja.
    fn extensions(&self) -> &'static [&'static str];
    /// Lee los campos del archivo. Vacío si no aplica o falla.
    fn read(&self, path: &Path) -> Vec<MetadataField>;
}

/// Proveedores registrados. Los de `core` se listan aquí; los de `platform` (p. ej. versión de
/// exe en Windows) se AGREGAN en runtime vía `register_provider` al arrancar la app.
use std::sync::RwLock;
static REGISTRY: RwLock<Vec<Box<dyn MetadataProvider>>> = RwLock::new(Vec::new());

/// Registra un proveedor (idempotencia no garantizada; llamar una vez por proveedor al iniciar).
pub fn register_provider(p: Box<dyn MetadataProvider>) {
    if let Ok(mut reg) = REGISTRY.write() {
        reg.push(p);
    }
}

/// Asegura que los proveedores de `core` estén registrados (imagen). Idempotente vía Once.
fn ensure_core_providers() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        register_provider(Box::new(image_meta::ImageMeta));
        register_provider(Box::new(audio_meta::AudioMeta));
        register_provider(Box::new(office_meta::OfficeMeta));
    });
}

/// Devuelve los campos de metadata del archivo `path` buscando un proveedor por su extensión.
/// Vacío si no hay proveedor o si falla. Es la entrada única que consume la UI.
pub fn metadata_for(path: &Path) -> Vec<MetadataField> {
    ensure_core_providers();
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_ascii_lowercase(),
        None => return Vec::new(),
    };
    let reg = match REGISTRY.read() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    for p in reg.iter() {
        if p.extensions().iter().any(|e| *e == ext) {
            let fields = p.read(path);
            if !fields.is_empty() {
                return fields;
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imagen_reporta_dimensiones() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.png");
        // PNG 4x2 mínimo con la crate image.
        let img = image::RgbaImage::new(4, 2);
        img.save(&p).unwrap();
        let fields = metadata_for(&p);
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.dimensions" && f.value == "4 × 2"),
            "debe reportar dimensiones 4 × 2: {fields:?}"
        );
    }

    #[test]
    fn sin_proveedor_devuelve_vacio() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.xyz");
        std::fs::write(&p, b"nada").unwrap();
        assert!(metadata_for(&p).is_empty());
    }
}
