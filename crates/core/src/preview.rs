// Naygo — clasificación y truncado para la vista previa liviana (puro, sin egui).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica PURA del panel Preview: decide qué tipo de vista previa admite una ruta
//! según las reglas configuradas (toggle + alias por extensión) y trunca el texto a un
//! tope de líneas y de bytes. No toca disco ni egui: la UI lee el archivo en un worker y
//! aplica esto.

use serde::{Deserialize, Serialize};

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

/// Regla de previsualización para UNA extensión: si se previsualiza y, opcionalmente,
/// como qué otra extensión tratarla (alias). Editable en Configuración.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewRule {
    /// Extensión sin punto, en minúscula ("sif", "txt", "png").
    pub ext: String,
    /// Si se previsualiza esta extensión.
    pub enabled: bool,
    /// Tratar la extensión como otra (alias). `Some("xml")` => un .sif se clasifica
    /// como .xml. `None` => por sí misma.
    pub treat_as: Option<String>,
}

/// Clasifica una ruta según las reglas configuradas. Resuelve el alias `treat_as` (un
/// salto, sin ciclar) y luego decide Text/Image/None por la extensión efectiva. Una
/// extensión sin regla, o con la regla deshabilitada, es `None`.
pub fn classify_rules(path: &std::path::Path, rules: &[PreviewRule]) -> PreviewKind {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ext.is_empty() {
        return PreviewKind::None;
    }
    let Some(rule) = rules.iter().find(|r| r.ext == ext) else {
        return PreviewKind::None;
    };
    if !rule.enabled {
        return PreviewKind::None;
    }
    // Extensión efectiva: el alias si lo hay (un salto), si no la propia.
    let effective = rule.treat_as.as_deref().unwrap_or(&ext);
    kind_of_extension(effective)
}

/// Tipo intrínseco de una extensión (sin reglas): imagen fija, o texto si está en la
/// lista semilla de texto, o None. Lo usa `classify_rules` tras resolver el alias.
fn kind_of_extension(ext: &str) -> PreviewKind {
    if IMAGE_EXTENSIONS.contains(&ext) {
        PreviewKind::Image
    } else if DEFAULT_TEXT_EXTENSIONS.contains(&ext) {
        PreviewKind::Text
    } else {
        PreviewKind::None
    }
}

/// Reglas por defecto: cada extensión de texto semilla + cada imagen, todas
/// habilitadas y sin alias.
pub fn default_preview_rules() -> Vec<PreviewRule> {
    let mk = |e: &&str| PreviewRule {
        ext: (*e).to_string(),
        enabled: true,
        treat_as: None,
    };
    DEFAULT_TEXT_EXTENSIONS
        .iter()
        .chain(IMAGE_EXTENSIONS.iter())
        .map(mk)
        .collect()
}

/// Migra un CSV viejo de extensiones de texto a reglas (cada una habilitada, sin
/// alias) + las reglas de imagen por defecto. Para settings.json previos (lote 2).
pub fn rules_from_csv(csv: &str) -> Vec<PreviewRule> {
    let mut rules: Vec<PreviewRule> = parse_text_extensions(csv)
        .into_iter()
        .map(|ext| PreviewRule {
            ext,
            enabled: true,
            treat_as: None,
        })
        .collect();
    // Agregar las imágenes que falten (habilitadas).
    for img in IMAGE_EXTENSIONS {
        if !rules.iter().any(|r| r.ext == *img) {
            rules.push(PreviewRule {
                ext: (*img).to_string(),
                enabled: true,
                treat_as: None,
            });
        }
    }
    rules
}

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

    #[test]
    fn classify_con_reglas_toggle_y_alias() {
        let rules = vec![
            PreviewRule {
                ext: "txt".into(),
                enabled: true,
                treat_as: None,
            },
            PreviewRule {
                ext: "log".into(),
                enabled: false,
                treat_as: None,
            },
            PreviewRule {
                ext: "sif".into(),
                enabled: true,
                treat_as: Some("xml".into()),
            },
            PreviewRule {
                ext: "xml".into(),
                enabled: true,
                treat_as: None,
            },
            PreviewRule {
                ext: "png".into(),
                enabled: true,
                treat_as: None,
            },
            PreviewRule {
                ext: "jpg".into(),
                enabled: false,
                treat_as: None,
            },
        ];
        assert_eq!(
            classify_rules(Path::new("a.txt"), &rules),
            PreviewKind::Text
        );
        // log deshabilitado -> None aunque sea texto.
        assert_eq!(
            classify_rules(Path::new("a.log"), &rules),
            PreviewKind::None
        );
        // sif (alias a xml) -> texto.
        assert_eq!(
            classify_rules(Path::new("a.sif"), &rules),
            PreviewKind::Text
        );
        // png habilitado -> imagen.
        assert_eq!(
            classify_rules(Path::new("a.PNG"), &rules),
            PreviewKind::Image
        );
        // jpg deshabilitado -> None.
        assert_eq!(
            classify_rules(Path::new("a.jpg"), &rules),
            PreviewKind::None
        );
        // extensión sin regla -> None.
        assert_eq!(
            classify_rules(Path::new("a.mp4"), &rules),
            PreviewKind::None
        );
    }

    #[test]
    fn classify_alias_a_imagen_y_alias_roto() {
        let rules = vec![
            PreviewRule {
                ext: "raw".into(),
                enabled: true,
                treat_as: Some("png".into()),
            },
            PreviewRule {
                ext: "png".into(),
                enabled: true,
                treat_as: None,
            },
            PreviewRule {
                ext: "weird".into(),
                enabled: true,
                treat_as: Some("zzz".into()),
            },
        ];
        // raw -> png -> imagen.
        assert_eq!(
            classify_rules(Path::new("a.raw"), &rules),
            PreviewKind::Image
        );
        // alias a una extensión sin tipo conocido -> None (un salto, no cicla).
        assert_eq!(
            classify_rules(Path::new("a.weird"), &rules),
            PreviewKind::None
        );
    }

    #[test]
    fn default_rules_tiene_texto_e_imagen_habilitados() {
        let rules = default_preview_rules();
        assert!(rules.iter().any(|r| r.ext == "txt" && r.enabled));
        assert!(rules.iter().any(|r| r.ext == "png" && r.enabled));
        assert!(rules.iter().all(|r| r.enabled && r.treat_as.is_none()));
    }

    #[test]
    fn migracion_csv_a_reglas() {
        let rules = rules_from_csv("txt, md, .RS,, json");
        assert!(rules
            .iter()
            .any(|r| r.ext == "txt" && r.enabled && r.treat_as.is_none()));
        assert!(rules.iter().any(|r| r.ext == "rs"));
        assert!(rules.iter().any(|r| r.ext == "png"));
    }
}
