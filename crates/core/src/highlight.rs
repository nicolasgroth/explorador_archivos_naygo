// Naygo — resaltado de sintaxis para la vista previa (syntect). Puro: sin UI ni Windows.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

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

    #[test]
    fn todos_los_lenguajes_resaltan_sin_panic() {
        // Cada variante debe encontrar su gramática (o degradar a texto plano) sin caerse.
        let langs = [
            CodeLang::Xml,
            CodeLang::Json,
            CodeLang::Html,
            CodeLang::Css,
            CodeLang::JavaScript,
            CodeLang::C,
            CodeLang::Cpp,
            CodeLang::Java,
            CodeLang::Python,
            CodeLang::Rust,
            CodeLang::Sql,
            CodeLang::Bash,
            CodeLang::Markdown,
            CodeLang::Yaml,
            CodeLang::Toml,
            CodeLang::Ini,
        ];
        for lang in langs {
            let lines = highlight("línea de prueba\nsegunda línea", lang);
            assert_eq!(lines.len(), 2, "{lang:?} debe devolver 2 líneas");
        }
    }

    #[test]
    fn rust_colorea_con_mas_de_un_color() {
        // "fn" es keyword: con el tema embebido debe haber al menos dos colores distintos.
        let lines = highlight("fn main() { let x = 1; }", CodeLang::Rust);
        let colores: std::collections::HashSet<(u8, u8, u8)> = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.color))
            .collect();
        assert!(
            colores.len() >= 2,
            "el resaltado de Rust debe usar más de un color: {colores:?}"
        );
    }

    #[test]
    fn unicode_y_emoji_no_panican_ni_se_pierden() {
        let src = "fn 🚀() -> &'static str { \"canción\" }";
        let lines = highlight(src, CodeLang::Rust);
        assert_eq!(lines.len(), 1);
        let rejoined: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(rejoined, src, "el texto unicode se conserva íntegro");
    }

    #[test]
    fn ultima_linea_sin_salto_cuenta_igual() {
        // "a\nb" (sin \n final) son 2 líneas; "a\nb\n" también (LinesWithEndings no agrega
        // una línea vacía extra por el salto terminal).
        assert_eq!(highlight("a\nb", CodeLang::Json).len(), 2);
        assert_eq!(highlight("a\nb\n", CodeLang::Json).len(), 2);
        // Líneas vacías intermedias se conservan como líneas (posiblemente sin spans).
        let lines = highlight("a\n\nb", CodeLang::Json);
        assert_eq!(lines.len(), 3);
    }
}
