// Naygo — parser del CHANGELOG: extrae las notas de una versión concreta.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Parser mínimo de un CHANGELOG con formato "Keep a Changelog":
//! encabezados de versión `## [X.Y.Z] — fecha`, subsecciones `### Categoría`
//! y viñetas `- …`. Sin dependencias: parseo línea a línea. Tolerante a
//! formatos imperfectos (nunca hace panic; ante algo inesperado devuelve
//! `None` o secciones vacías). Lo consume la UI para la sección "Novedades".

/// Una subsección de notas (una categoría con sus viñetas).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteSection {
    /// Nombre de la categoría tal cual aparece tras `### ` (p. ej. "Añadido").
    pub category: String,
    /// Viñetas de la categoría, sin el "- " inicial.
    pub items: Vec<String>,
}

/// Notas de una versión del CHANGELOG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseNotes {
    /// Versión tal cual aparece entre corchetes en `## [..]`.
    pub version: String,
    /// Fecha si el encabezado la incluye tras un guion (p. ej. "2026-06-18").
    pub date: Option<String>,
    /// Subsecciones en el orden en que aparecen.
    pub sections: Vec<NoteSection>,
}

/// Extrae del texto de un CHANGELOG el bloque de la versión `version`.
///
/// Busca un encabezado `## [<version>]` (la coincidencia es por el contenido
/// EXACTO entre corchetes). Devuelve `None` si no existe tal bloque. El bloque
/// termina en el siguiente `## ` o al final del texto.
pub fn release_notes(changelog: &str, version: &str) -> Option<ReleaseNotes> {
    let mut lines = changelog.lines();
    // Encontrar el encabezado de la versión pedida.
    let header = lines
        .by_ref()
        .find(|l| version_in_header(l).is_some_and(|v| v == version))?;
    let date = date_in_header(header);

    let mut sections: Vec<NoteSection> = Vec::new();
    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("## ") {
            break; // empezó la siguiente versión
        }
        if let Some(cat) = trimmed.strip_prefix("### ") {
            sections.push(NoteSection {
                category: cat.trim().to_string(),
                items: Vec::new(),
            });
        } else if let Some(item) = trimmed.strip_prefix("- ") {
            if let Some(last) = sections.last_mut() {
                last.items.push(item.trim().to_string());
            }
            // Una viñeta antes de cualquier `### ` se ignora (formato raro).
        }
    }

    Some(ReleaseNotes {
        version: version.to_string(),
        date,
        sections,
    })
}

/// Si la línea es un encabezado `## [algo] …`, devuelve `algo` (lo de dentro de
/// los corchetes). Si no, `None`.
fn version_in_header(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("## ")?;
    let rest = rest.trim_start();
    let inner = rest.strip_prefix('[')?;
    let end = inner.find(']')?;
    Some(inner[..end].trim())
}

/// Extrae la fecha de un encabezado de versión, si viene tras un guion
/// (acepta "—" o "-"). Devuelve el texto tras el guion, recortado.
fn date_in_header(line: &str) -> Option<String> {
    // Tomar lo que viene después del `]`.
    let after = line.split_once(']')?.1.trim();
    // Quitar un guion inicial (em-dash o normal) y espacios.
    let after = after.trim_start_matches('—').trim_start_matches('-').trim();
    if after.is_empty() {
        None
    } else {
        Some(after.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Changelog

## [Sin publicar]

## [0.2.0] — 2026-07-01
### Añadido
- Vista profunda recursiva.
- Copiar rutas absolutas.
### Corregido
- Fuga de z-order en el encabezado.

## [0.1.0] — 2026-06-18
### Añadido
- Navegación tipo Commander.
";

    #[test]
    fn extrae_la_version_pedida_e_ignora_otras() {
        let n = release_notes(SAMPLE, "0.2.0").expect("debe encontrar 0.2.0");
        assert_eq!(n.version, "0.2.0");
        assert_eq!(n.date.as_deref(), Some("2026-07-01"));
        assert_eq!(n.sections.len(), 2);
        assert_eq!(n.sections[0].category, "Añadido");
        assert_eq!(
            n.sections[0].items,
            vec![
                "Vista profunda recursiva.".to_string(),
                "Copiar rutas absolutas.".to_string()
            ]
        );
        assert_eq!(n.sections[1].category, "Corregido");
        assert_eq!(
            n.sections[1].items,
            vec!["Fuga de z-order en el encabezado.".to_string()]
        );
    }

    #[test]
    fn version_inexistente_devuelve_none() {
        assert!(release_notes(SAMPLE, "9.9.9").is_none());
    }

    #[test]
    fn changelog_vacio_o_sin_encabezados_devuelve_none() {
        assert!(release_notes("", "0.1.0").is_none());
        assert!(release_notes("texto suelto sin secciones", "0.1.0").is_none());
    }

    #[test]
    fn bloque_sin_vinetas_da_secciones_vacias_sin_panic() {
        let cl = "## [1.0.0]\n### Añadido\n";
        let n = release_notes(cl, "1.0.0").expect("encuentra 1.0.0");
        assert_eq!(n.date, None);
        assert_eq!(n.sections.len(), 1);
        assert!(n.sections[0].items.is_empty());
    }

    #[test]
    fn no_se_mezclan_vinetas_de_la_version_siguiente() {
        // 0.1.0 es el último bloque: solo su categoría/viñeta, nada de 0.2.0.
        let n = release_notes(SAMPLE, "0.1.0").expect("encuentra 0.1.0");
        assert_eq!(n.sections.len(), 1);
        assert_eq!(
            n.sections[0].items,
            vec!["Navegación tipo Commander.".to_string()]
        );
    }
}
