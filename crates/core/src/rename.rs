// Naygo — selección del rename inline (ciclo F2): qué parte del nombre se selecciona. Puro.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! El editor de rename inline cicla la selección con F2: 1ª pulsación = nombre sin
//! extensión, 2ª = solo extensión, 3ª = todo. Aquí va solo el cálculo de rangos (puro,
//! testeable); la UI lo aplica al editor. Mismo comportamiento que la capa egui.

/// Rango `(inicio, fin)` en CHARS a seleccionar en el editor de rename, según la etapa del
/// ciclo F2: 0 = nombre sin extensión, 1 = solo extensión, 2+ = todo. Sin extensión válida
/// (carpetas, dotfiles tipo ".gitignore") cualquier etapa selecciona todo.
pub fn rename_selection(text: &str, stage: u8) -> (usize, usize) {
    let total = text.chars().count();
    let split = text
        .rsplit_once('.')
        .filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty());
    match (stage, split) {
        (0, Some((stem, _))) => (0, stem.chars().count()),
        (1, Some((stem, _))) => (stem.chars().count() + 1, total),
        _ => (0, total),
    }
}

/// Variante para controles que expresan la selección como offsets UTF-8 en bytes (Slint).
/// Mantener la conversión aquí evita cortar un carácter multibyte en nombres con tildes,
/// ideogramas o emoji.
pub fn rename_selection_byte_offsets(text: &str, stage: u8) -> (usize, usize) {
    let (start, end) = rename_selection(text, stage);
    let char_to_byte = |char_pos: usize| {
        text.char_indices()
            .nth(char_pos)
            .map_or(text.len(), |(byte_pos, _)| byte_pos)
    };
    (char_to_byte(start), char_to_byte(end))
}

#[cfg(test)]
mod tests {
    use super::{rename_selection, rename_selection_byte_offsets};

    #[test]
    fn ciclo_nombre_ext_todo() {
        assert_eq!(rename_selection("foto.png", 0), (0, 4)); // "foto"
        assert_eq!(rename_selection("foto.png", 1), (5, 8)); // "png"
        assert_eq!(rename_selection("foto.png", 2), (0, 8)); // todo
                                                             // Sin extensión válida → siempre todo.
        assert_eq!(rename_selection("carpeta", 0), (0, 7));
        assert_eq!(rename_selection(".gitignore", 0), (0, 10));
        // Punto final sin extensión real → todo (ext vacía).
        assert_eq!(rename_selection("raro.", 0), (0, 5));
        // Nombre con varios puntos: el split es por el ÚLTIMO punto.
        assert_eq!(rename_selection("a.b.txt", 0), (0, 3)); // "a.b"
        assert_eq!(rename_selection("a.b.txt", 1), (4, 7)); // "txt"
    }

    #[test]
    fn offsets_para_slint_respetan_utf8() {
        // "canción" son 7 chars, pero 8 bytes por la ó.
        assert_eq!(rename_selection_byte_offsets("canción.txt", 0), (0, 8));
        assert_eq!(rename_selection_byte_offsets("canción.txt", 1), (9, 12));
        assert_eq!(rename_selection_byte_offsets("日本語.pdf", 0), (0, 9));
    }

    #[test]
    fn etapas_sin_extension_valida_siempre_seleccionan_todo() {
        // Sin extensión (o dotfile), TODAS las etapas del ciclo seleccionan todo.
        for stage in [0u8, 1, 2, 3] {
            assert_eq!(rename_selection("carpeta", stage), (0, 7));
            assert_eq!(rename_selection(".gitignore", stage), (0, 10));
        }
    }

    #[test]
    fn etapas_mas_alla_del_ciclo_seleccionan_todo() {
        // stage 2+ = todo, sin importar qué tan alto sea el contador.
        assert_eq!(rename_selection("foto.png", 3), (0, 8));
        assert_eq!(rename_selection("foto.png", 255), (0, 8));
    }

    #[test]
    fn cadena_vacia_da_rango_vacio() {
        assert_eq!(rename_selection("", 0), (0, 0));
        assert_eq!(rename_selection("", 1), (0, 0));
        assert_eq!(rename_selection_byte_offsets("", 0), (0, 0));
    }

    #[test]
    fn solo_punto_no_corta_nada() {
        // "." se parte en ("", "") → sin extensión válida → todo (rango vacío).
        assert_eq!(rename_selection(".", 0), (0, 1));
        assert_eq!(rename_selection(".", 1), (0, 1));
    }

    #[test]
    fn offsets_con_emoji_respetan_utf8() {
        // "🎉" es 1 char pero 4 bytes: stage 0 selecciona solo el emoji (4 bytes).
        assert_eq!(rename_selection_byte_offsets("🎉.txt", 0), (0, 4));
        // stage 1 selecciona "txt": empieza tras "🎉." (5 bytes) hasta el final (8).
        assert_eq!(rename_selection_byte_offsets("🎉.txt", 1), (5, 8));
        // stage 2 = todo, en bytes.
        assert_eq!(rename_selection_byte_offsets("🎉.txt", 2), (0, 8));
    }
}
