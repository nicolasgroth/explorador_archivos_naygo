// Naygo — selección del rename inline (ciclo F2): qué parte del nombre se selecciona. Puro.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

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

#[cfg(test)]
mod tests {
    use super::rename_selection;

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
}
