// Naygo — texto del preview de comprimidos: encabezado de totales + árbol ASCII.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Construye el TEXTO de la vista previa de un archivo comprimido (zip/tar): un encabezado
//! con totales (N archivos, M carpetas, tamaño) y un árbol ASCII indentado (├─ └─ │) del
//! contenido. Puro y testeable: recibe las entradas ya leídas (sin tocar disco). Determinista.

/// Una entrada de un archivo comprimido: ruta interna + tamaño descomprimido.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveEntry {
    /// Ruta interna con `/` como separador, p.ej. "proyecto/src/main.rs".
    pub path: String,
    pub is_dir: bool,
    /// Tamaño descomprimido en bytes (0 para carpetas).
    pub size: u64,
}

/// Resumen de un archivo comprimido (para el encabezado).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ArchiveSummary {
    pub files: usize,
    pub dirs: usize,
    pub total_uncompressed: u64,
    /// Si se listaron menos entradas que las reales (se aplicó un tope).
    pub truncated: bool,
    pub total_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_default_es_cero() {
        let s = ArchiveSummary::default();
        assert_eq!(s.files, 0);
        assert_eq!(s.dirs, 0);
        assert_eq!(s.total_uncompressed, 0);
        assert!(!s.truncated);
    }
}
