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

/// Un nodo del árbol del archivo: una carpeta (con hijos) o un archivo (hoja).
#[derive(Debug)]
pub struct TreeNode {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    fn new_dir(name: &str) -> TreeNode {
        TreeNode { name: name.to_string(), is_dir: true, size: 0, children: Vec::new() }
    }
}

/// Construye el árbol a partir de las entradas planas. Crea carpetas intermedias implícitas
/// (rutas presentes en un archivo pero sin entrada propia). Ordena cada nivel: carpetas
/// primero, luego archivos, alfabético (case-insensitive). Devuelve el nodo raíz (sin nombre).
pub fn build_tree(entries: &[ArchiveEntry]) -> TreeNode {
    let mut root = TreeNode::new_dir("");
    for e in entries {
        let comps: Vec<&str> = e.path.split('/').filter(|s| !s.is_empty()).collect();
        if comps.is_empty() { continue; }
        let mut cur = &mut root;
        for (i, comp) in comps.iter().enumerate() {
            let last = i + 1 == comps.len();
            let want_file = last && !e.is_dir;
            let pos = cur.children.iter().position(|c| c.name == *comp);
            let idx = match pos {
                Some(p) => p,
                None => {
                    let node = if want_file {
                        TreeNode { name: comp.to_string(), is_dir: false, size: e.size, children: Vec::new() }
                    } else {
                        TreeNode::new_dir(comp)
                    };
                    cur.children.push(node);
                    cur.children.len() - 1
                }
            };
            if want_file {
                cur.children[idx].is_dir = false;
                cur.children[idx].size = e.size;
            }
            cur = &mut cur.children[idx];
        }
    }
    sort_tree(&mut root);
    root
}

fn sort_tree(node: &mut TreeNode) {
    node.children.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for c in &mut node.children { sort_tree(c); }
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

    #[test]
    fn build_tree_crea_carpetas_implicitas_y_ordena() {
        let entries = vec![
            ArchiveEntry { path: "a/b/c.txt".into(), is_dir: false, size: 10 },
            ArchiveEntry { path: "a/z.txt".into(), is_dir: false, size: 20 },
            ArchiveEntry { path: "a/b/".into(), is_dir: true, size: 0 },
        ];
        let root = build_tree(&entries);
        assert_eq!(root.children.len(), 1);
        let a = &root.children[0];
        assert_eq!(a.name, "a");
        assert!(a.is_dir);
        assert_eq!(a.children.len(), 2);
        assert_eq!(a.children[0].name, "b");
        assert!(a.children[0].is_dir);
        assert_eq!(a.children[1].name, "z.txt");
        assert!(!a.children[1].is_dir);
        assert_eq!(a.children[0].children.len(), 1);
        assert_eq!(a.children[0].children[0].name, "c.txt");
        assert_eq!(a.children[0].children[0].size, 10);
    }
}
