// Naygo — heurística pura: ¿el renderer OpenGL es por software? (sin egui/Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! La capa `ui` lee el nombre del renderer (`GL_RENDERER`) del contexto glow; esta
//! función PURA decide si es un rasterizador por software (sin GPU). Se usa para activar
//! el modo de bajo consumo en `Auto`. Testeable sin egui.

/// `true` si el nombre del renderer corresponde a un rasterizador por SOFTWARE (sin GPU
/// real): llvmpipe (Mesa), SwiftShader, el "Microsoft Basic Render Driver"/"GDI Generic"
/// de Windows, o softpipe. La comparación es insensible a mayúsculas.
pub fn is_software_renderer(renderer_name: &str) -> bool {
    let n = renderer_name.to_lowercase();
    [
        "llvmpipe",
        "softpipe",
        "software",
        "swiftshader",
        "microsoft basic render",
        "gdi generic",
    ]
    .iter()
    .any(|m| n.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_software() {
        assert!(is_software_renderer("llvmpipe (LLVM 15.0.7, 256 bits)"));
        assert!(is_software_renderer("SwiftShader Device"));
        assert!(is_software_renderer("Microsoft Basic Render Driver"));
        assert!(is_software_renderer("GDI Generic"));
        assert!(is_software_renderer("softpipe"));
    }

    #[test]
    fn no_marca_gpu_real() {
        assert!(!is_software_renderer("NVIDIA GeForce RTX 3060/PCIe/SSE2"));
        assert!(!is_software_renderer("Intel(R) UHD Graphics 620"));
        assert!(!is_software_renderer("AMD Radeon RX 580"));
        assert!(!is_software_renderer("ANGLE (Intel, Direct3D11)"));
    }

    #[test]
    fn case_insensitive() {
        assert!(is_software_renderer("LLVMPIPE"));
    }
}
