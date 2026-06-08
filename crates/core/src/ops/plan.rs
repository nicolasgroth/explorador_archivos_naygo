// Naygo — planificación de operaciones: expandir a pasos + validar (recorre FS).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `plan` toma una `OpRequest` y produce un `OpPlan` (lista de pasos + totales),
//! validando precondiciones (nombres, carpeta-dentro-de-sí-misma). Para Copy/Move
//! recorre el árbol de orígenes leyendo tamaños. Devuelve `Result<OpPlan, PlanError>`.

use super::names::is_valid_name;
use super::{OpKind, OpPlan, OpRequest, OpStep};
use std::path::{Path, PathBuf};

/// Error de planificación (antes de empezar a ejecutar).
#[derive(Debug, PartialEq)]
pub enum PlanError {
    /// El destino está dentro de uno de los orígenes (copia recursiva infinita).
    DestInsideSource,
    /// Nombre inválido (al renombrar/crear).
    InvalidName(String),
    /// Falta el destino para una op que lo requiere.
    MissingDest,
    /// Un origen no existe / no se pudo leer.
    SourceUnreadable(PathBuf),
}

/// Planifica una `OpRequest`: produce los pasos + totales, o un `PlanError`.
pub fn plan(req: &OpRequest) -> Result<OpPlan, PlanError> {
    match &req.kind {
        OpKind::Copy | OpKind::Move => plan_transfer(req),
        OpKind::Delete { .. } => plan_delete(req),
        OpKind::Rename { new_name } => {
            if !is_valid_name(new_name) {
                return Err(PlanError::InvalidName(new_name.clone()));
            }
            let from = req.sources.first().cloned();
            let to = from
                .as_ref()
                .and_then(|p| p.parent())
                .map(|parent| parent.join(new_name))
                .ok_or(PlanError::MissingDest)?;
            Ok(OpPlan {
                steps: vec![OpStep { from, to, bytes: 0, is_dir: false }],
                total_bytes: 0,
                total_files: 1,
            })
        }
        OpKind::CreateDir { name } | OpKind::CreateFile { name } => {
            if !is_valid_name(name) {
                return Err(PlanError::InvalidName(name.clone()));
            }
            let dest = req.dest_dir.clone().ok_or(PlanError::MissingDest)?;
            let is_dir = matches!(req.kind, OpKind::CreateDir { .. });
            Ok(OpPlan {
                steps: vec![OpStep { from: None, to: dest.join(name), bytes: 0, is_dir }],
                total_bytes: 0,
                total_files: if is_dir { 0 } else { 1 },
            })
        }
    }
}

fn plan_transfer(req: &OpRequest) -> Result<OpPlan, PlanError> {
    let dest = req.dest_dir.clone().ok_or(PlanError::MissingDest)?;
    for src in &req.sources {
        if src.is_dir() && is_inside(&dest, src) {
            return Err(PlanError::DestInsideSource);
        }
    }
    let mut steps = Vec::new();
    let mut total_bytes = 0u64;
    let mut total_files = 0usize;
    for src in &req.sources {
        let base_to = dest.join(src.file_name().unwrap_or_default());
        expand(src, &base_to, &mut steps, &mut total_bytes, &mut total_files)?;
    }
    Ok(OpPlan { steps, total_bytes, total_files })
}

fn plan_delete(req: &OpRequest) -> Result<OpPlan, PlanError> {
    let mut steps = Vec::new();
    for src in &req.sources {
        if !src.exists() {
            return Err(PlanError::SourceUnreadable(src.clone()));
        }
        let is_dir = src.is_dir();
        steps.push(OpStep { from: Some(src.clone()), to: src.clone(), bytes: 0, is_dir });
    }
    let n = steps.iter().filter(|s| !s.is_dir).count();
    Ok(OpPlan { steps, total_bytes: 0, total_files: n })
}

fn expand(
    src: &Path,
    to: &Path,
    steps: &mut Vec<OpStep>,
    total_bytes: &mut u64,
    total_files: &mut usize,
) -> Result<(), PlanError> {
    let meta = std::fs::metadata(src).map_err(|_| PlanError::SourceUnreadable(src.to_path_buf()))?;
    if meta.is_dir() {
        steps.push(OpStep { from: Some(src.to_path_buf()), to: to.to_path_buf(), bytes: 0, is_dir: true });
        let entries = std::fs::read_dir(src).map_err(|_| PlanError::SourceUnreadable(src.to_path_buf()))?;
        for entry in entries.flatten() {
            let child = entry.path();
            let child_to = to.join(entry.file_name());
            expand(&child, &child_to, steps, total_bytes, total_files)?;
        }
    } else {
        let bytes = meta.len();
        steps.push(OpStep { from: Some(src.to_path_buf()), to: to.to_path_buf(), bytes, is_dir: false });
        *total_bytes += bytes;
        *total_files += 1;
    }
    Ok(())
}

/// `true` si `inner` está dentro de (o es igual a) `outer`.
fn is_inside(inner: &Path, outer: &Path) -> bool {
    inner.starts_with(outer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{ConflictPolicy, OpKind, OpRequest};
    use std::fs;

    fn req(kind: OpKind, sources: Vec<PathBuf>, dest: Option<PathBuf>) -> OpRequest {
        OpRequest { kind, sources, dest_dir: dest, conflict: ConflictPolicy::Overwrite }
    }

    #[test]
    fn copy_archivo_simple_un_paso_con_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"hola").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let plan = plan(&req(OpKind::Copy, vec![src.clone()], Some(dest.clone()))).unwrap();
        assert_eq!(plan.total_files, 1);
        assert_eq!(plan.total_bytes, 4);
        assert_eq!(plan.steps[0].to, dest.join("a.txt"));
        assert_eq!(plan.steps[0].bytes, 4);
        assert!(!plan.steps[0].is_dir);
    }

    #[test]
    fn copy_carpeta_recursiva_expande_pasos() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"aa").unwrap();
        fs::create_dir(src.join("sub")).unwrap();
        fs::write(src.join("sub/b.txt"), b"bbb").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let plan = plan(&req(OpKind::Copy, vec![src], Some(dest))).unwrap();
        assert_eq!(plan.total_bytes, 5);
        assert_eq!(plan.total_files, 2);
        assert!(plan.steps.iter().any(|s| s.is_dir));
    }

    #[test]
    fn copy_carpeta_dentro_de_si_misma_es_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        let dest = src.join("sub");
        fs::create_dir(&dest).unwrap();
        let e = plan(&req(OpKind::Copy, vec![src], Some(dest))).unwrap_err();
        assert_eq!(e, PlanError::DestInsideSource);
    }

    #[test]
    fn rename_nombre_invalido_es_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let r = req(OpKind::Rename { new_name: "a/b.txt".into() }, vec![src], None);
        let e = plan(&r).unwrap_err();
        assert!(matches!(e, PlanError::InvalidName(_)));
    }

    #[test]
    fn copy_sin_dest_es_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let e = plan(&req(OpKind::Copy, vec![src], None)).unwrap_err();
        assert_eq!(e, PlanError::MissingDest);
    }
}
