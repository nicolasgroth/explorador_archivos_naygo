// Naygo — matching de texto para el filtro visual por tipeo (contiene, sin tildes ni caso).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Comparación de nombres de archivo para el filtro "contiene" del typeahead:
//! case-insensitive E insensible a tildes/diacríticos ("cancion" matchea
//! "canción"; la ñ se pliega a "n" como hace Explorer con su comparación
//! lingüística: al buscar, que "nino" encuentre "niño" es lo esperado).
//! La aguja se plega UNA vez (`fold_for_match`) y los nombres se pliegan por
//! comparación; en carpetas enormes el costo es un fold por nombre por rebuild
//! de filas (no por tick: el rebuild solo corre cuando cambia la firma).

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Pliega `s` para comparación: NFD → sin marcas combinantes (tildes, diéresis,
/// cedillas) → minúsculas. La NFD de 'ñ' es 'n' + U+0303, así que queda "n":
/// es a propósito (filtro amigable, no purista).
pub fn fold_for_match(s: &str) -> String {
    s.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// ¿`name` contiene `needle_folded` (ya plegada con `fold_for_match`)?
pub fn contains_folded(name: &str, needle_folded: &str) -> bool {
    if needle_folded.is_empty() {
        return false;
    }
    fold_for_match(name).contains(needle_folded)
}

/// ¿`name` empieza con `needle_folded` (ya plegada)? Disponible para quien prefiera un
/// salto estilo Explorer (prefijo); el filtro visual usa `contains_folded`/`match_char_range`.
pub fn starts_with_folded(name: &str, needle_folded: &str) -> bool {
    if needle_folded.is_empty() {
        return false;
    }
    fold_for_match(name).starts_with(needle_folded)
}

/// Rango del primer match de `needle_folded` en `name`, en ÍNDICES DE CHAR del nombre
/// ORIGINAL (no del plegado): `(inicio, fin_exclusivo)`. None si no hay match.
///
/// Sirve para pintar el tramo exacto que coincidió (el filtro visual resalta el texto
/// matcheado dentro del nombre). El índice (plegado + fronteras char→byte) se cachea por
/// nombre en el hilo (`INDEX_CACHE`): al tipear con el filtro activo las filas se
/// re-matchean en cada tecla, y así cada nombre se pliega UNA vez por carpeta en vez de
/// una por rebuild. Limitación conocida y aceptada: el lowercase se hace por char aislado,
/// así que casos contextuales raros (la sigma griega final 'ς') pliegan distinto que el
/// lowercase de cadena — irrelevante para nombres de archivo reales.
pub fn match_char_range(name: &str, needle_folded: &str) -> Option<(usize, usize)> {
    if needle_folded.is_empty() {
        return None;
    }
    let index = index_of(name);
    let start_f = index.folded.find(needle_folded)?;
    let end_f = start_f + needle_folded.len();
    // Primer char cuyo plegado TERMINA después de start_f = char donde empieza el match.
    let start_c = index.boundaries.iter().position(|&b| b > start_f)?;
    // El match termina en el primer char cuyo plegado cubre end_f (fin exclusivo = índice+1).
    let end_c = index
        .boundaries
        .iter()
        .position(|&b| b >= end_f)
        .map(|i| i + 1)
        .unwrap_or(index.boundaries.len());
    Some((start_c, end_c))
}

/// Índice pre-computado de un nombre para matching: su plegado y, por cada char del
/// original, el largo en bytes del plegado acumulado (para mapear el rango de vuelta).
struct NameIndex {
    folded: String,
    boundaries: Vec<usize>,
}

/// Tope de seguridad del caché de índices (entradas). Una carpeta gigante son ~100k
/// nombres; pasado el tope se vacía entero (simple) — el peor caso es re-plegar una vez.
const INDEX_CACHE_CAP: usize = 150_000;

thread_local! {
    /// Caché nombre → índice, por HILO (solo el hilo de UI matchea; los workers no). Sin
    /// Mutex: `RefCell` basta. Se vacía al navegar (`clear_index_cache`, la llama
    /// `start_listing`) para no arrastrar nombres de carpetas viejas.
    static INDEX_CACHE: std::cell::RefCell<std::collections::HashMap<String, std::rc::Rc<NameIndex>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Vacía el caché de índices de nombres (al navegar a otra carpeta).
pub fn clear_index_cache() {
    INDEX_CACHE.with(|c| c.borrow_mut().clear());
}

/// Índice de `name`, construido una vez por nombre por hilo (los clones del `Rc` son gratis).
fn index_of(name: &str) -> std::rc::Rc<NameIndex> {
    INDEX_CACHE.with(|c| {
        if let Some(ix) = c.borrow().get(name) {
            return ix.clone();
        }
        let mut folded = String::new();
        let mut boundaries: Vec<usize> = Vec::with_capacity(name.len() / 2 + 1);
        for ch in name.chars() {
            for fc in ch
                .nfd()
                .filter(|x| !is_combining_mark(*x))
                .flat_map(char::to_lowercase)
            {
                folded.push(fc);
            }
            boundaries.push(folded.len());
        }
        let ix = std::rc::Rc::new(NameIndex { folded, boundaries });
        let mut map = c.borrow_mut();
        if map.len() >= INDEX_CACHE_CAP {
            map.clear();
        }
        map.insert(name.to_string(), ix.clone());
        ix
    })
}

/// Corta `name` en (pre, match, post) según el rango de chars `(start, end)` de
/// `match_char_range`. Listo para pintar los tres tramos sin saber de offsets.
pub fn split_at_char_range(name: &str, start: usize, end: usize) -> (String, String, String) {
    let pre: String = name.chars().take(start).collect();
    let mid: String = name.chars().skip(start).take(end - start).collect();
    let post: String = name.chars().skip(end).collect();
    (pre, mid, post)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_quita_tildes_y_baja_caso() {
        assert_eq!(fold_for_match("Canción"), "cancion");
        assert_eq!(fold_for_match("MÜLLER"), "muller");
        assert_eq!(fold_for_match("garçon"), "garcon");
    }

    #[test]
    fn contains_es_case_y_acento_insensible() {
        // La aguja va PLEGADA (contrato): quien llama la pliega una vez con fold_for_match.
        assert!(contains_folded(
            "Canción Final.mp3",
            &fold_for_match("cancion")
        ));
        assert!(contains_folded("canciones", &fold_for_match("CIÓN")));
        assert!(contains_folded(
            "LK-300TV_Full.jpg",
            &fold_for_match("_full")
        ));
        assert!(!contains_folded("readme.txt", &fold_for_match("xyz")));
    }

    #[test]
    fn aguja_vacia_no_matchea() {
        assert!(!contains_folded("algo", ""));
        assert!(!starts_with_folded("algo", ""));
        assert_eq!(match_char_range("algo", ""), None);
    }

    #[test]
    fn starts_with_respeta_prefijo() {
        assert!(starts_with_folded("Álbum 01", "album"));
        assert!(!starts_with_folded("mi Álbum", "album"));
    }

    #[test]
    fn match_range_apunta_al_tramo_en_el_nombre_original() {
        // "canción": la 'ó' son dos chars en NFD pero UNO en el original; el rango debe
        // venir en índices del original para poder cortar/pintar el tramo real.
        let needle = fold_for_match("ció");
        let (s, e) = match_char_range("Canción Final.mp3", &needle).unwrap();
        let (pre, mid, post) = split_at_char_range("Canción Final.mp3", s, e);
        assert_eq!(
            (pre.as_str(), mid.as_str(), post.as_str()),
            ("Can", "ció", "n Final.mp3")
        );
    }

    #[test]
    fn match_range_sin_match_da_none_y_vacia_es_none() {
        assert_eq!(match_char_range("alpha.txt", &fold_for_match("zzz")), None);
        assert_eq!(match_char_range("alpha.txt", ""), None);
    }

    #[test]
    fn match_range_al_inicio_y_al_final() {
        let n1 = fold_for_match("lk");
        assert_eq!(match_char_range("LK-300TV.jpg", &n1), Some((0, 2)));
        let n2 = fold_for_match("jpg");
        let (s, e) = match_char_range("LK-300TV.jpg", &n2).unwrap();
        assert_eq!((s, e), (9, 12));
        let (_p, mid, post) = split_at_char_range("LK-300TV.jpg", s, e);
        assert_eq!(mid, "jpg");
        assert_eq!(post, "");
    }

    #[test]
    fn match_range_con_enie_plegada() {
        // "niño" pliega a "nino": el match de "nino" cubre 4 chars del ORIGINAL (ñ = 1 char).
        let needle = fold_for_match("nino");
        let (s, e) = match_char_range("el niño bonito.png", &needle).unwrap();
        let (_p, mid, _q) = split_at_char_range("el niño bonito.png", s, e);
        assert_eq!(mid, "niño");
    }

    #[test]
    fn nino_con_tilde_matchea_nino_con_enie() {
        // Decisión de diseño documentada arriba: el filtro es amigable, no purista.
        // (Aguja plegada, como manda el contrato de contains_folded.)
        assert!(contains_folded("niño.png", &fold_for_match("nino")));
        assert!(contains_folded("nino.png", &fold_for_match("niño")));
    }

    #[test]
    fn el_cache_de_indices_no_cambia_resultados() {
        // Mismo nombre dos veces (la segunda pasa por el caché del hilo): idéntico resultado.
        let needle = fold_for_match("ció");
        let first = match_char_range("Canción Final.mp3", &needle);
        let second = match_char_range("Canción Final.mp3", &needle);
        assert_eq!(first, second);
        // Tras vaciar el caché (navegación), sigue siendo correcto.
        clear_index_cache();
        assert_eq!(match_char_range("Canción Final.mp3", &needle), first);
    }
}
