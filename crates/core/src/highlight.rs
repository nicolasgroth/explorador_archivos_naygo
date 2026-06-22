// Naygo — resaltado de sintaxis para la vista previa (syntect). Puro: sin UI ni Windows.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Convierte un fragmento de código (que YA debe venir recortado por `crate::preview`:
//! ≤ TEXT_MAX_LINES líneas y ≤ TEXT_MAX_LINE_CHARS por línea) en líneas → segmentos
//! coloreados. La UI los pinta como un `Text` por segmento. El set de gramáticas y el tema
//! están embebidos en syntect; el color es independiente del tema de Naygo (el fondo del
//! panel sigue siendo el del tema activo).

use crate::preview::CodeLang;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Un segmento de texto coloreado dentro de una línea (RGB para la UI).
#[derive(Clone, Debug, PartialEq)]
pub struct HlSpan {
    pub text: String,
    pub color: (u8, u8, u8),
}

/// Una línea resaltada = lista de segmentos (en orden de izquierda a derecha).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct HlLine {
    pub spans: Vec<HlSpan>,
}

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Theme> = OnceLock::new();

fn syntaxes() -> &'static SyntaxSet {
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> &'static Theme {
    THEME.get_or_init(|| {
        let ts = ThemeSet::load_defaults();
        // Tema oscuro, legible y de buen contraste sobre el fondo del panel.
        ts.themes["base16-ocean.dark"].clone()
    })
}

/// Token de búsqueda de la gramática de syntect para cada lenguaje (una extensión típica).
fn syntax_token(lang: CodeLang) -> &'static str {
    match lang {
        CodeLang::Xml => "xml",
        CodeLang::Json => "json",
        CodeLang::Html => "html",
        CodeLang::Css => "css",
        CodeLang::JavaScript => "js",
        CodeLang::C => "c",
        CodeLang::Cpp => "cpp",
        CodeLang::Java => "java",
        CodeLang::Python => "py",
        CodeLang::Rust => "rs",
        CodeLang::Sql => "sql",
        CodeLang::Bash => "sh",
        CodeLang::Markdown => "md",
        CodeLang::Yaml => "yaml",
        CodeLang::Toml => "toml",
        CodeLang::Ini => "ini",
    }
}

/// Resalta `text` como `lang`, una `HlLine` por línea. Degrada (cada línea = un span gris)
/// si la gramática no existe o syntect falla; nunca paniquea. `text` debe venir recortado.
pub fn highlight(text: &str, lang: CodeLang) -> Vec<HlLine> {
    let ss = syntaxes();
    let syntax = ss
        .find_syntax_by_extension(syntax_token(lang))
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let mut h = HighlightLines::new(syntax, theme());
    let mut out = Vec::new();
    for line in LinesWithEndings::from(text) {
        let spans = match h.highlight_line(line, ss) {
            Ok(ranges) => ranges
                .into_iter()
                .map(|(style, piece): (Style, &str)| HlSpan {
                    text: piece.trim_end_matches('\n').to_string(),
                    color: (style.foreground.r, style.foreground.g, style.foreground.b),
                })
                .filter(|s| !s.text.is_empty())
                .collect(),
            Err(_) => vec![HlSpan {
                text: line.trim_end_matches('\n').to_string(),
                color: (200, 200, 200),
            }],
        };
        out.push(HlLine { spans });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resalta_json_en_lineas_con_segmentos() {
        let src = "{\n  \"a\": 1\n}";
        let lines = highlight(src, CodeLang::Json);
        assert_eq!(lines.len(), 3, "una HlLine por línea de entrada");
        // El texto concatenado de cada línea reconstruye la línea original (sin el \n).
        let rejoined: String = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(rejoined, src);
    }

    #[test]
    fn degrada_sin_panic_con_texto_raro() {
        // Vacío: 0 o 1 líneas, sin panic.
        let v = highlight("", CodeLang::Rust);
        assert!(v.len() <= 1);
        // Texto que no es del lenguaje: no panica, una línea.
        let lines = highlight("esto no es rust válido <<<", CodeLang::Rust);
        assert_eq!(lines.len(), 1);
    }
}
