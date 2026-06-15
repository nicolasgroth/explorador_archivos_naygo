// Naygo — construcción de OpRequest desde la selección/disparadores (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Funciones puras que arman una `OpRequest` a partir de las rutas seleccionadas y el
//! destino. Viven en core (sin UI) para reusarse desde cualquier capa (egui y Slint).

use super::{ConflictPolicy, OpKind, OpRequest};
use std::path::PathBuf;

/// Copiar/mover `sources` a `dest_dir`.
pub fn transfer(kind_move: bool, sources: Vec<PathBuf>, dest_dir: PathBuf) -> OpRequest {
    OpRequest {
        kind: if kind_move {
            OpKind::Move
        } else {
            OpKind::Copy
        },
        sources,
        dest_dir: Some(dest_dir),
        conflict: ConflictPolicy::Ask,
    }
}

/// Eliminar `sources` (a papelera o permanente).
pub fn delete(sources: Vec<PathBuf>, to_trash: bool) -> OpRequest {
    OpRequest {
        kind: OpKind::Delete { to_trash },
        sources,
        dest_dir: None,
        conflict: ConflictPolicy::Overwrite,
    }
}

/// Renombrar un archivo.
pub fn rename(source: PathBuf, new_name: String) -> OpRequest {
    OpRequest {
        kind: OpKind::Rename { new_name },
        sources: vec![source],
        dest_dir: None,
        conflict: ConflictPolicy::Ask,
    }
}

/// Crear carpeta/archivo en `dir`.
pub fn create(dir: PathBuf, name: String, is_dir: bool) -> OpRequest {
    OpRequest {
        kind: if is_dir {
            OpKind::CreateDir { name }
        } else {
            OpKind::CreateFile { name }
        },
        sources: vec![],
        dest_dir: Some(dir),
        conflict: ConflictPolicy::Ask,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_move_arma_kind_move() {
        let r = transfer(true, vec![PathBuf::from("a")], PathBuf::from("dst"));
        assert_eq!(r.kind, OpKind::Move);
        assert_eq!(r.dest_dir, Some(PathBuf::from("dst")));
    }

    #[test]
    fn transfer_copy_es_ask() {
        let r = transfer(false, vec![PathBuf::from("a")], PathBuf::from("dst"));
        assert_eq!(r.kind, OpKind::Copy);
        assert_eq!(r.conflict, ConflictPolicy::Ask);
    }

    #[test]
    fn delete_papelera_flag() {
        let r = delete(vec![PathBuf::from("a")], true);
        assert_eq!(r.kind, OpKind::Delete { to_trash: true });
    }

    #[test]
    fn rename_arma_kind_rename() {
        let r = rename(PathBuf::from("a.txt"), "b.txt".into());
        assert_eq!(r.kind, OpKind::Rename { new_name: "b.txt".into() });
        assert_eq!(r.sources, vec![PathBuf::from("a.txt")]);
    }

    #[test]
    fn create_dir_vs_file() {
        assert_eq!(
            create(PathBuf::from("d"), "x".into(), true).kind,
            OpKind::CreateDir { name: "x".into() }
        );
        assert_eq!(
            create(PathBuf::from("d"), "x".into(), false).kind,
            OpKind::CreateFile { name: "x".into() }
        );
    }
}
