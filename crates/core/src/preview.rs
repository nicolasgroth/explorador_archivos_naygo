// Naygo — clasificación y truncado para la vista previa liviana (puro, sin egui).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica PURA del panel Preview: decide qué tipo de vista previa admite una ruta
//! según su extensión (texto / imagen / nada) y trunca el texto a un tope de líneas y
//! de bytes. No toca disco ni egui: la UI lee el archivo en un worker y aplica esto.

/// El tipo de vista previa que admite un archivo, decidido por su extensión.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    /// Texto plano (se muestran las primeras líneas en monospace).
    Text,
    /// Imagen liviana (se decodifica y se muestra escalada).
    Image,
    /// Sin vista previa (video/audio/binario/desconocido): NUNCA se lee el archivo.
    None,
}

/// Extensiones de imagen previsualizables. Fijas en v1 (el decoder de `image` soporta
/// todas: png/jpeg/bmp/gif/webp/ico).
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp", "gif", "webp", "ico"];

/// Extensiones de texto previsualizables por defecto. La UI permite editar la lista en
/// Configuración; esta es la semilla.
pub const DEFAULT_TEXT_EXTENSIONS: &[&str] = &[
    "txt", "log", "md", "json", "xml", "csv", "toml", "yaml", "yml", "ini", "rs", "py", "js",
    "html",
];

/// Tope de líneas que se muestran de un archivo de texto.
pub const TEXT_MAX_LINES: usize = 100;
/// Tope de bytes que se leen de un archivo de texto (lo que llegue primero con las líneas).
pub const TEXT_MAX_BYTES: usize = 64 * 1024;
/// Tope de bytes de una imagen para intentar decodificarla (más grande → "muy grande").
pub const IMAGE_MAX_BYTES: u64 = 20 * 1024 * 1024;
/// Lado máximo (px) de la textura de la imagen: si excede, se reescala antes de subirla.
pub const IMAGE_MAX_SIDE: u32 = 1024;

/// Extrae la extensión de una ruta en minúsculas (sin punto). Vacío si no tiene.
fn extension_lower(path: &std::path::Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Clasifica una ruta según su extensión y la lista de extensiones de texto configurada.
/// Las imágenes son fijas (`IMAGE_EXTENSIONS`); el resto del trabajo (leer/decodificar)
/// lo hace el worker. `text_exts` debe venir en minúsculas.
pub fn classify(path: &std::path::Path, text_exts: &[String]) -> PreviewKind {
    let ext = extension_lower(path);
    if ext.is_empty() {
        return PreviewKind::None;
    }
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return PreviewKind::Image;
    }
    if text_exts.iter().any(|e| e == &ext) {
        return PreviewKind::Text;
    }
    PreviewKind::None
}

/// Parsea la lista de extensiones de texto desde un String separado por comas (como se
/// guarda en Settings). Normaliza a minúsculas, quita puntos/espacios y vacíos.
pub fn parse_text_extensions(csv: &str) -> Vec<String> {
    csv.split(',')
        .map(|s| s.trim().trim_start_matches('.').to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// La lista de extensiones de texto por defecto como CSV (para el default del Setting).
pub fn default_text_extensions_csv() -> String {
    DEFAULT_TEXT_EXTENSIONS.join(", ")
}

/// Resultado de truncar un texto para la vista previa.
pub struct TruncatedText {
    /// El texto a mostrar (a lo más `TEXT_MAX_LINES` líneas).
    pub text: String,
    /// Si se truncó (por líneas o por bytes): la UI agrega un aviso final.
    pub truncated: bool,
}

/// Trunca `bytes` (conversión lossy desde UTF-8) a las primeras `TEXT_MAX_LINES` líneas.
/// Marca `truncated` si había más líneas que el tope O si el buffer ya venía cortado por
/// bytes (`hit_byte_cap`). No agrega el aviso: eso es presentación (i18n) de la UI.
pub fn truncate_text(bytes: &[u8], hit_byte_cap: bool) -> TruncatedText {
    let s = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let mut more_lines = false;
    for (i, line) in s.lines().enumerate() {
        if i >= TEXT_MAX_LINES {
            more_lines = true;
            break;
        }
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line);
    }
    TruncatedText {
        text: out,
        truncated: more_lines || hit_byte_cap,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn clasifica_imagen_texto_y_nada() {
        let texts: Vec<String> = parse_text_extensions("txt,md,rs");
        assert_eq!(classify(Path::new("a.PNG"), &texts), PreviewKind::Image);
        assert_eq!(classify(Path::new("a.jpg"), &texts), PreviewKind::Image);
        assert_eq!(classify(Path::new("a.txt"), &texts), PreviewKind::Text);
        assert_eq!(classify(Path::new("a.MD"), &texts), PreviewKind::Text);
        // Extensión de texto NO configurada → None.
        assert_eq!(classify(Path::new("a.json"), &texts), PreviewKind::None);
        // Video/binario → None.
        assert_eq!(classify(Path::new("a.mp4"), &texts), PreviewKind::None);
        assert_eq!(classify(Path::new("a.exe"), &texts), PreviewKind::None);
        // Sin extensión → None.
        assert_eq!(classify(Path::new("LICENSE"), &texts), PreviewKind::None);
    }

    #[test]
    fn parse_extensiones_normaliza() {
        let v = parse_text_extensions("  TXT , .md ,, json ,");
        assert_eq!(v, vec!["txt", "md", "json"]);
    }

    #[test]
    fn default_csv_round_trip() {
        let csv = default_text_extensions_csv();
        let parsed = parse_text_extensions(&csv);
        assert_eq!(parsed.len(), DEFAULT_TEXT_EXTENSIONS.len());
        assert!(parsed.contains(&"txt".to_string()));
        assert!(parsed.contains(&"json".to_string()));
    }

    #[test]
    fn trunca_por_lineas() {
        // 150 líneas → se muestran 100 y se marca truncado.
        let src: String = (0..150).map(|i| format!("línea {i}\n")).collect();
        let t = truncate_text(src.as_bytes(), false);
        assert_eq!(t.text.lines().count(), TEXT_MAX_LINES);
        assert!(t.truncated);
        assert!(t.text.starts_with("línea 0"));
    }

    #[test]
    fn no_trunca_si_cabe() {
        let src = "una\ndos\ntres";
        let t = truncate_text(src.as_bytes(), false);
        assert_eq!(t.text, "una\ndos\ntres");
        assert!(!t.truncated);
    }

    #[test]
    fn marca_truncado_si_se_corto_por_bytes() {
        // Pocas líneas pero el buffer venía cortado por el tope de bytes.
        let src = "una\ndos";
        let t = truncate_text(src.as_bytes(), true);
        assert!(t.truncated, "el corte por bytes también marca truncado");
    }

    #[test]
    fn texto_no_utf8_es_lossy_sin_panic() {
        let bytes = [0xff, 0xfe, b'h', b'i', b'\n'];
        let t = truncate_text(&bytes, false);
        assert!(t.text.contains("hi"));
    }
}
