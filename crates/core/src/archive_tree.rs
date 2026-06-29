// Naygo — texto del preview de comprimidos: encabezado de totales + árbol ASCII.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Construye el TEXTO de la vista previa de un archivo comprimido (zip/tar): un encabezado
//! con totales (N archivos, M carpetas, tamaño) y un árbol ASCII indentado (├─ └─ │) del
//! contenido. Puro y testeable: recibe las entradas ya leídas (sin tocar disco). Determinista.

use crate::format::{format_size, SizeFormat};

/// Textos traducidos para el encabezado/pie del árbol. La capa de UI los pasa ya resueltos
/// (core no conoce el catálogo i18n). `and_more` es una plantilla con `{n}`.
#[derive(Clone, Debug)]
pub struct ArchiveLabels {
    pub files: String,
    pub folders: String,
    pub uncompressed: String,
    pub more_entries: String,
    pub and_more: String,
}

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
            if want_file && cur.children[idx].children.is_empty() {
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

/// Texto del preview: encabezado de totales + árbol ASCII. Puro y determinista.
pub fn render_archive_tree(
    entries: &[ArchiveEntry],
    summary: &ArchiveSummary,
    name: &str,
    size_fmt: SizeFormat,
    labels: &ArchiveLabels,
) -> String {
    let mut out = String::new();
    out.push_str(name);
    out.push('\n');
    out.push_str(&format!(
        "{} {}, {} {} · {} {}\n",
        summary.files, labels.files, summary.dirs, labels.folders,
        format_size(summary.total_uncompressed, size_fmt), labels.uncompressed,
    ));
    out.push_str("──────────────────────────────\n");
    let root = build_tree(entries);
    render_children(&root.children, "", &mut out, size_fmt);
    if summary.truncated {
        let extra = summary.total_entries.saturating_sub(entries.len());
        if extra > 0 {
            out.push_str(&format!("\n{}\n", labels.and_more.replace("{n}", &extra.to_string())));
        } else {
            // tar truncado: no conocemos el total real sin leerlo entero, así que el texto es
            // honesto (hay más, sin afirmar un número que no sabemos).
            out.push_str(&format!("\n{}\n", labels.more_entries));
        }
    }
    out
}

fn render_children(children: &[TreeNode], prefix: &str, out: &mut String, size_fmt: SizeFormat) {
    let n = children.len();
    for (i, node) in children.iter().enumerate() {
        let last = i + 1 == n;
        let connector = if last { "└─ " } else { "├─ " };
        out.push_str(prefix);
        out.push_str(connector);
        out.push_str(&node.name);
        if node.is_dir {
            out.push('/');
        } else {
            out.push_str(&format!("  {}", format_size(node.size, size_fmt)));
        }
        out.push('\n');
        if node.is_dir && !node.children.is_empty() {
            let child_prefix = format!("{}{}", prefix, if last { "   " } else { "│  " });
            render_children(&node.children, &child_prefix, out, size_fmt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels_es() -> ArchiveLabels {
        ArchiveLabels {
            files: "archivo(s)".into(),
            folders: "carpeta(s)".into(),
            uncompressed: "sin comprimir".into(),
            more_entries: "… y más entradas".into(),
            and_more: "… y {n} más".into(),
        }
    }

    #[test]
    fn summary_default_es_cero() {
        let s = ArchiveSummary::default();
        assert_eq!(s.files, 0);
        assert_eq!(s.dirs, 0);
        assert_eq!(s.total_uncompressed, 0);
        assert!(!s.truncated);
    }

    #[test]
    fn render_incluye_encabezado_y_arbol_ascii() {
        use crate::format::SizeFormat;
        let entries = vec![
            ArchiveEntry { path: "p/src/main.rs".into(), is_dir: false, size: 4300 },
            ArchiveEntry { path: "p/README.md".into(), is_dir: false, size: 2100 },
        ];
        let summary = ArchiveSummary { files: 2, dirs: 2, total_uncompressed: 6400, truncated: false, total_entries: 2 };
        let out = render_archive_tree(&entries, &summary, "demo.zip", SizeFormat::Auto, &labels_es());
        assert!(out.contains("2 archivo"));
        assert!(out.contains("carpeta"));
        assert!(out.contains("├─ ") || out.contains("└─ "));
        assert!(out.contains("main.rs"));
        assert!(out.contains("README.md"));
        assert!(out.contains("└─"));
    }

    #[test]
    fn render_truncado_agrega_y_n_mas() {
        use crate::format::SizeFormat;
        let summary = ArchiveSummary { files: 1, dirs: 0, total_uncompressed: 5, truncated: true, total_entries: 600 };
        let entries = vec![ArchiveEntry { path: "a.txt".into(), is_dir: false, size: 5 }];
        let out = render_archive_tree(&entries, &summary, "big.zip", SizeFormat::Auto, &labels_es());
        assert!(out.contains("599"));
        assert!(out.contains("más"));
    }

    #[test]
    fn render_lista_vacia_no_panica() {
        use crate::format::SizeFormat;
        let out = render_archive_tree(&[], &ArchiveSummary::default(), "vacio.zip", SizeFormat::Auto, &labels_es());
        assert!(out.contains("0 archivo"));
    }

    #[test]
    fn render_usa_los_labels_dados() {
        let labels = ArchiveLabels {
            files: "file(s)".into(),
            folders: "folder(s)".into(),
            uncompressed: "uncompressed".into(),
            more_entries: "… and more entries".into(),
            and_more: "… and {n} more".into(),
        };
        let entries = vec![ArchiveEntry { path: "a.txt".into(), is_dir: false, size: 5 }];
        let summary = ArchiveSummary { files: 1, dirs: 0, total_uncompressed: 5, truncated: false, total_entries: 1 };
        let out = render_archive_tree(&entries, &summary, "x.zip", SizeFormat::Auto, &labels);
        assert!(out.contains("file(s)"));
        assert!(out.contains("uncompressed"));
        assert!(!out.contains("archivo"), "no debe haber español hardcodeado");
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

    #[test]
    fn build_tree_archivo_malformado_no_corrompe_carpeta() {
        // Archivo (malformado) con "a/b/c.txt" Y "a/b" como ARCHIVO: la carpeta b con su hijo
        // c.txt no se debe corromper (el contenido no se pierde).
        let entries = vec![
            ArchiveEntry { path: "a/b/c.txt".into(), is_dir: false, size: 10 },
            ArchiveEntry { path: "a/b".into(), is_dir: false, size: 99 },
        ];
        let root = build_tree(&entries);
        let a = &root.children[0];
        let b = &a.children[0];
        assert_eq!(b.name, "b");
        assert!(b.is_dir, "b sigue siendo carpeta (tiene un hijo)");
        assert_eq!(b.children.len(), 1, "c.txt no se perdió");
        assert_eq!(b.children[0].name, "c.txt");
    }
}
