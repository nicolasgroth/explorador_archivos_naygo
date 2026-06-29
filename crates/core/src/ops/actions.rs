// Naygo — construcción de OpRequest desde la selección/disparadores (puro).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

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

/// Renombrar en lote: `sources[i]` → `new_names[i]` (mismo directorio). El plan ordena por
/// dependencia (shifts) y es una sola op deshacible. El preview del diálogo ya validó.
pub fn batch_rename(sources: Vec<PathBuf>, new_names: Vec<String>) -> OpRequest {
    OpRequest {
        kind: OpKind::BatchRename { new_names },
        sources,
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

/// Una línea del cuadro "nueva(s) carpeta(s)" ya validada: el nombre relativo a crear
/// (puede ser anidado, p. ej. `a\b\c`) o el motivo por el que se rechazó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderSpec {
    /// Ruta relativa válida (componentes separados por `\`). Lista para `create_dir_all`.
    Valid(String),
    /// Línea inválida: el texto original + el motivo (clave i18n) para avisar al usuario.
    Invalid {
        line: String,
        reason: NewFolderError,
    },
}

/// Por qué se rechazó una línea del cuadro de nuevas carpetas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewFolderError {
    /// Algún componente quedó vacío (p. ej. `a\\b` o termina en `\`).
    EmptyComponent,
    /// Un componente tiene caracteres no permitidos en Windows (`<>:"|?*` o control).
    InvalidChars,
    /// Un componente es `.` o `..` (no se permite navegar fuera).
    Traversal,
}

/// Parsea el texto multilínea del cuadro "nueva(s) carpeta(s)": cada línea no vacía es una
/// carpeta; el separador `\` (o `/`) la vuelve anidada. Devuelve un `FolderSpec` por línea
/// (válida o con motivo), preservando el orden. No toca el disco: es puro y testeable.
pub fn parse_new_folders(text: &str) -> Vec<FolderSpec> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|line| match validate_folder_line(line) {
            Ok(rel) => FolderSpec::Valid(rel),
            Err(reason) => FolderSpec::Invalid {
                line: line.to_string(),
                reason,
            },
        })
        .collect()
}

/// Valida una línea y devuelve la ruta relativa normalizada (componentes unidos por `\`).
fn validate_folder_line(line: &str) -> Result<String, NewFolderError> {
    // Aceptar tanto `\` (Windows) como `/` como separador de anidamiento.
    let parts: Vec<&str> = line.split(['\\', '/']).collect();
    let mut clean: Vec<String> = Vec::with_capacity(parts.len());
    for raw in parts {
        let part = raw.trim();
        if part.is_empty() {
            return Err(NewFolderError::EmptyComponent);
        }
        if part == "." || part == ".." {
            return Err(NewFolderError::Traversal);
        }
        // Caracteres prohibidos por Windows en nombres de archivo/carpeta.
        if part
            .chars()
            .any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') || (c as u32) < 0x20)
        {
            return Err(NewFolderError::InvalidChars);
        }
        clean.push(part.to_string());
    }
    Ok(clean.join("\\"))
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
        assert_eq!(
            r.kind,
            OpKind::Rename {
                new_name: "b.txt".into()
            }
        );
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

    #[test]
    fn parse_new_folders_simples_y_anidadas() {
        let specs = parse_new_folders("uno\ndos\\tres\ncuatro/cinco");
        assert_eq!(
            specs,
            vec![
                FolderSpec::Valid("uno".into()),
                FolderSpec::Valid("dos\\tres".into()),
                // El separador `/` se normaliza a `\`.
                FolderSpec::Valid("cuatro\\cinco".into()),
            ]
        );
    }

    #[test]
    fn parse_new_folders_ignora_vacias_y_recorta() {
        let specs = parse_new_folders("\n  hola  \n\n   \n");
        assert_eq!(specs, vec![FolderSpec::Valid("hola".into())]);
    }

    #[test]
    fn parse_new_folders_rechaza_chars_invalidos() {
        let specs = parse_new_folders("a:b");
        assert_eq!(
            specs,
            vec![FolderSpec::Invalid {
                line: "a:b".into(),
                reason: NewFolderError::InvalidChars
            }]
        );
    }

    #[test]
    fn parse_new_folders_rechaza_componente_vacio() {
        let specs = parse_new_folders("a\\\\b");
        assert_eq!(
            specs,
            vec![FolderSpec::Invalid {
                line: "a\\\\b".into(),
                reason: NewFolderError::EmptyComponent
            }]
        );
    }

    #[test]
    fn parse_new_folders_rechaza_traversal() {
        let specs = parse_new_folders("a\\..\\b");
        assert_eq!(
            specs,
            vec![FolderSpec::Invalid {
                line: "a\\..\\b".into(),
                reason: NewFolderError::Traversal
            }]
        );
    }
}
