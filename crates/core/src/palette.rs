// Naygo — paleta de comandos: modelo de comandos y fuzzy-match (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica PURA de la paleta de comandos (sin UI ni Windows). La UI arma la lista de
//! `Command` desde sus fuentes (acciones, archivos, recientes, favoritos, temas) y usa
//! `filter_and_rank` para filtrar/ordenar según lo que el usuario escribe. 100% testeable.

use crate::keymap::Action;
use crate::theme::ThemeId;
use std::path::PathBuf;

/// Categoría de un comando (define el ícono y la etiqueta en la UI).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandCategory {
    Action,
    File,
    Recent,
    Favorite,
    Theme,
    Config,
}

/// Qué ejecuta un comando al elegirlo.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandPayload {
    /// Una acción del keymap; se rutea por el dispatcher de teclado existente.
    Action(Action),
    /// Navegar el panel activo a esta ruta (reciente/favorito).
    Navigate(PathBuf),
    /// Enfocar/seleccionar un entry YA cargado en el panel activo, por su índice de VISTA.
    FocusEntry(usize),
    /// Aplicar este tema.
    Theme(ThemeId),
    /// Abrir la ventana de configuración.
    OpenConfig,
}

/// Un comando de la paleta.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    /// Texto a mostrar (ya traducido por la UI).
    pub label: String,
    pub category: CommandCategory,
    /// Atajo legible ("Ctrl+C"); solo acciones con atajo. Vacío si no tiene.
    pub shortcut: String,
    pub payload: CommandPayload,
}

/// Resultado de filtrar: índice del comando + score + posiciones (char-index) que matchearon.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandMatch {
    pub index: usize,
    pub score: i32,
    pub hit_positions: Vec<usize>,
}

/// Match fuzzy de subsecuencia con ranking. Devuelve `(score, hit_positions)` o `None` si la
/// query no es subsecuencia de `text`. Case-insensitive. Score más alto = mejor coincidencia:
/// se premia el match contiguo, el inicio de palabra y el prefijo; se penaliza la dispersión.
pub fn fuzzy_match(query: &str, text: &str) -> Option<(i32, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    let q: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
    let t: Vec<char> = text.chars().collect();
    let t_lower: Vec<char> = text.chars().flat_map(|c| c.to_lowercase()).collect();

    let mut hits = Vec::with_capacity(q.len());
    let mut qi = 0usize;
    let mut score = 0i32;
    let mut prev_match: Option<usize> = None;

    for (ti, &tc) in t_lower.iter().enumerate() {
        if qi >= q.len() {
            break;
        }
        if tc == q[qi] {
            hits.push(ti);
            if ti == 0 {
                score += 15;
            } else if matches!(
                t.get(ti - 1),
                Some(' ') | Some('_') | Some('-') | Some('\\') | Some('/') | Some('.')
            ) {
                score += 10;
            }
            if let Some(p) = prev_match {
                if ti == p + 1 {
                    score += 8;
                } else {
                    score -= (ti - p - 1).min(10) as i32;
                }
            }
            score += 1;
            prev_match = Some(ti);
            qi += 1;
        }
    }

    if qi == q.len() {
        Some((score, hits))
    } else {
        None
    }
}

/// Filtra los comandos por la query y los ordena por score (desc). Query vacía → todos, en el
/// orden original (la UI puede recortar a una "lista por defecto").
pub fn filter_and_rank(commands: &[Command], query: &str) -> Vec<CommandMatch> {
    let mut matches: Vec<CommandMatch> = commands
        .iter()
        .enumerate()
        .filter_map(|(index, c)| {
            fuzzy_match(query, &c.label).map(|(score, hit_positions)| CommandMatch {
                index,
                score,
                hit_positions,
            })
        })
        .collect();
    matches.sort_by(|a, b| b.score.cmp(&a.score).then(a.index.cmp(&b.index)));
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(label: &str) -> Command {
        Command {
            label: label.to_string(),
            category: CommandCategory::Action,
            shortcut: String::new(),
            payload: CommandPayload::OpenConfig,
        }
    }

    #[test]
    fn match_de_prefijo() {
        let (_score, hits) = fuzzy_match("cop", "Copiar").expect("debe matchear");
        assert_eq!(hits, vec![0, 1, 2]);
    }

    #[test]
    fn match_disperso_subsecuencia() {
        let m = fuzzy_match("cpo", "Copiar al otro panel");
        assert!(m.is_some(), "subsecuencia debe matchear");
    }

    #[test]
    fn no_match_si_falta_una_letra() {
        assert!(fuzzy_match("xyz", "Copiar").is_none());
    }

    #[test]
    fn case_insensitive() {
        assert!(fuzzy_match("COP", "copiar").is_some());
        assert!(fuzzy_match("cop", "COPIAR").is_some());
    }

    #[test]
    fn prefijo_puntua_mas_que_disperso() {
        let (prefix_score, _) = fuzzy_match("co", "Copiar").unwrap();
        let (sparse_score, _) = fuzzy_match("co", "Calcular tamañO").unwrap();
        assert!(prefix_score > sparse_score, "prefijo {prefix_score} > disperso {sparse_score}");
    }

    #[test]
    fn filter_rank_ordena_por_score_y_query_vacia_da_todos() {
        let cmds = vec![cmd("Calcular tamaño"), cmd("Copiar"), cmd("Cortar")];
        let all = filter_and_rank(&cmds, "");
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].index, 0);
        let res = filter_and_rank(&cmds, "co");
        assert!(res.iter().any(|m| m.index == 1), "Copiar debe estar");
        assert!(res[0].score >= res[res.len() - 1].score);
    }
}
