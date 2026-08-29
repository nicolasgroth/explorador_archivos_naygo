// Naygo — planificación pura y cancelable de sincronización entre carpetas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Recorre dos árboles fuera del hilo UI y produce un plan explícito. No ejecuta cambios.

use crate::CancellationToken;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncMode {
    LeftToRight,
    RightToLeft,
    Bidirectional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncAction {
    CopyLeftToRight,
    CopyRightToLeft,
    DeleteLeft,
    DeleteRight,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncItem {
    pub relative: PathBuf,
    pub action: SyncAction,
    pub bytes: u64,
    pub is_dir: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPlan {
    pub left_root: PathBuf,
    pub right_root: PathBuf,
    pub items: Vec<SyncItem>,
    pub copy_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncOptions {
    pub mode: SyncMode,
    pub delete_extras: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            mode: SyncMode::LeftToRight,
            delete_extras: false,
        }
    }
}

impl SyncPlan {
    /// Convierte las copias seleccionadas en un único plan de motor con destinos exactos.
    pub fn copy_op_plan(&self, selected: &[bool]) -> crate::ops::OpPlan {
        let mut steps = Vec::new();
        let mut total_bytes = 0_u64;
        let mut total_files = 0_usize;
        for (index, item) in self.items.iter().enumerate() {
            if !selected.get(index).copied().unwrap_or(false) {
                continue;
            }
            let (from, to) = match item.action {
                SyncAction::CopyLeftToRight => (
                    self.left_root.join(&item.relative),
                    self.right_root.join(&item.relative),
                ),
                SyncAction::CopyRightToLeft => (
                    self.right_root.join(&item.relative),
                    self.left_root.join(&item.relative),
                ),
                _ => continue,
            };
            steps.push(crate::ops::OpStep {
                from: Some(from),
                to,
                bytes: item.bytes,
                is_dir: item.is_dir,
            });
            if !item.is_dir {
                total_files += 1;
                total_bytes = total_bytes.saturating_add(item.bytes);
            }
        }
        crate::ops::OpPlan {
            steps,
            total_bytes,
            total_files,
            pre_delete: Vec::new(),
        }
    }

    /// Rutas extra seleccionadas para enviar a la papelera con el flujo normal de borrado.
    pub fn delete_paths(&self, selected: &[bool]) -> Vec<PathBuf> {
        self.items
            .iter()
            .enumerate()
            .filter(|(index, _)| selected.get(*index).copied().unwrap_or(false))
            .filter_map(|(_, item)| match item.action {
                SyncAction::DeleteLeft => Some(self.left_root.join(&item.relative)),
                SyncAction::DeleteRight => Some(self.right_root.join(&item.relative)),
                _ => None,
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncError {
    SameFolder,
    Unreadable(PathBuf),
    Cancelled,
}

#[derive(Clone, Debug)]
struct ItemMeta {
    size: u64,
    modified: Option<SystemTime>,
    is_dir: bool,
}

pub fn plan_sync(
    left_root: &Path,
    right_root: &Path,
    options: SyncOptions,
    token: &CancellationToken,
) -> Result<SyncPlan, SyncError> {
    if same_path(left_root, right_root) {
        return Err(SyncError::SameFolder);
    }
    let left = inventory(left_root, token)?;
    let right = inventory(right_root, token)?;
    let mut names = BTreeSet::new();
    names.extend(left.keys().cloned());
    names.extend(right.keys().cloned());
    let mut items = Vec::new();

    for key in names {
        if token.is_cancelled() {
            return Err(SyncError::Cancelled);
        }
        let l = left.get(&key);
        let r = right.get(&key);
        let relative = l
            .map(|(path, _)| path.clone())
            .or_else(|| r.map(|(path, _)| path.clone()))
            .unwrap_or_default();
        let action = decide(l.map(|(_, m)| m), r.map(|(_, m)| m), options);
        if let Some(action) = action {
            let meta = match action {
                SyncAction::CopyLeftToRight | SyncAction::DeleteLeft => l.map(|(_, m)| m),
                SyncAction::CopyRightToLeft | SyncAction::DeleteRight => r.map(|(_, m)| m),
                SyncAction::Conflict => l.map(|(_, m)| m).or_else(|| r.map(|(_, m)| m)),
            };
            let Some(meta) = meta else { continue };
            // No planificar carpetas que tienen contenido: sus hijos crearán los padres. Sí
            // conservar carpetas vacías, detectadas abajo al compactar.
            items.push(SyncItem {
                relative,
                action,
                bytes: if meta.is_dir { 0 } else { meta.size },
                is_dir: meta.is_dir,
            });
        }
    }

    compact_directories(&mut items);
    let copy_bytes = items
        .iter()
        .filter(|item| {
            matches!(
                item.action,
                SyncAction::CopyLeftToRight | SyncAction::CopyRightToLeft
            )
        })
        .map(|item| item.bytes)
        .sum();
    Ok(SyncPlan {
        left_root: left_root.to_path_buf(),
        right_root: right_root.to_path_buf(),
        items,
        copy_bytes,
    })
}

fn decide(
    left: Option<&ItemMeta>,
    right: Option<&ItemMeta>,
    options: SyncOptions,
) -> Option<SyncAction> {
    use SyncAction::*;
    match (left, right, options.mode) {
        (Some(_), None, SyncMode::LeftToRight) => Some(CopyLeftToRight),
        (None, Some(_), SyncMode::LeftToRight) if options.delete_extras => Some(DeleteRight),
        (None, Some(_), SyncMode::LeftToRight) => None,
        (None, Some(_), SyncMode::RightToLeft) => Some(CopyRightToLeft),
        (Some(_), None, SyncMode::RightToLeft) if options.delete_extras => Some(DeleteLeft),
        (Some(_), None, SyncMode::RightToLeft) => None,
        (Some(_), None, SyncMode::Bidirectional) => Some(CopyLeftToRight),
        (None, Some(_), SyncMode::Bidirectional) => Some(CopyRightToLeft),
        (Some(l), Some(r), _) if equivalent(l, r) => None,
        (Some(l), Some(r), SyncMode::LeftToRight) if l.is_dir == r.is_dir => Some(CopyLeftToRight),
        (Some(l), Some(r), SyncMode::RightToLeft) if l.is_dir == r.is_dir => Some(CopyRightToLeft),
        (Some(l), Some(r), SyncMode::Bidirectional) if l.is_dir == r.is_dir => {
            match newer(l.modified, r.modified) {
                Some(std::cmp::Ordering::Greater) => Some(CopyLeftToRight),
                Some(std::cmp::Ordering::Less) => Some(CopyRightToLeft),
                _ => Some(Conflict),
            }
        }
        (Some(_), Some(_), _) => Some(Conflict),
        (None, None, _) => None,
    }
}

fn equivalent(left: &ItemMeta, right: &ItemMeta) -> bool {
    if left.is_dir != right.is_dir {
        return false;
    }
    if left.is_dir {
        return true;
    }
    left.size == right.size
        && match (left.modified, right.modified) {
            (Some(a), Some(b)) => duration_between(a, b) <= Duration::from_secs(2),
            _ => false,
        }
}

fn newer(left: Option<SystemTime>, right: Option<SystemTime>) -> Option<std::cmp::Ordering> {
    let (Some(left), Some(right)) = (left, right) else {
        return None;
    };
    if duration_between(left, right) <= Duration::from_secs(2) {
        None
    } else {
        Some(left.cmp(&right))
    }
}

fn duration_between(a: SystemTime, b: SystemTime) -> Duration {
    a.duration_since(b)
        .or_else(|_| b.duration_since(a))
        .unwrap_or_default()
}

type Inventory = HashMap<String, (PathBuf, ItemMeta)>;

fn inventory(root: &Path, token: &CancellationToken) -> Result<Inventory, SyncError> {
    let metadata = fs::metadata(root).map_err(|_| SyncError::Unreadable(root.to_path_buf()))?;
    if !metadata.is_dir() {
        return Err(SyncError::Unreadable(root.to_path_buf()));
    }
    let mut out = HashMap::new();
    walk(root, root, token, &mut out)?;
    Ok(out)
}

fn walk(
    root: &Path,
    dir: &Path,
    token: &CancellationToken,
    out: &mut Inventory,
) -> Result<(), SyncError> {
    if token.is_cancelled() {
        return Err(SyncError::Cancelled);
    }
    let entries = fs::read_dir(dir).map_err(|_| SyncError::Unreadable(dir.to_path_buf()))?;
    for entry in entries {
        if token.is_cancelled() {
            return Err(SyncError::Cancelled);
        }
        let entry = entry.map_err(|_| SyncError::Unreadable(dir.to_path_buf()))?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|_| SyncError::Unreadable(path.clone()))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| SyncError::Unreadable(path.clone()))?
            .to_path_buf();
        out.insert(
            relative_key(&relative),
            (
                relative,
                ItemMeta {
                    size: if meta.is_file() { meta.len() } else { 0 },
                    modified: meta.modified().ok(),
                    is_dir: meta.is_dir(),
                },
            ),
        );
        if meta.is_dir() {
            walk(root, &path, token, out)?;
        }
    }
    Ok(())
}

fn compact_directories(items: &mut Vec<SyncItem>) {
    let snapshot = items.clone();
    items.retain(|item| {
        // Un descendiente de una carpeta que se borrará sobra: enviar la carpeta superior a la
        // papelera incluye todo y evita duplicados/conflictos de orden.
        let under_deleted_dir = snapshot.iter().any(|parent| {
            parent.is_dir
                && parent.relative != item.relative
                && item.relative.starts_with(&parent.relative)
                && parent.action == item.action
                && matches!(
                    parent.action,
                    SyncAction::DeleteLeft | SyncAction::DeleteRight
                )
        });
        if under_deleted_dir {
            return false;
        }
        if !item.is_dir
            || matches!(
                item.action,
                SyncAction::DeleteLeft | SyncAction::DeleteRight
            )
        {
            return true;
        }
        !snapshot.iter().any(|child| {
            child.relative != item.relative
                && child.relative.starts_with(&item.relative)
                && child.action == item.action
        })
    });
}

fn relative_key(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

fn same_path(left: &Path, right: &Path) -> bool {
    relative_key(left) == relative_key(right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{
        engine::run_plan, ConflictAction, ConflictDecision, ConflictPolicy, OpKind, OpMsg,
    };
    use std::io::Write;
    use std::sync::mpsc;

    #[test]
    fn unidireccional_planifica_nuevos_y_no_borra_por_defecto() {
        let tmp = tempfile::tempdir().unwrap();
        let left = tmp.path().join("left");
        let right = tmp.path().join("right");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::File::create(left.join("nuevo.txt"))
            .unwrap()
            .write_all(b"hola")
            .unwrap();
        fs::write(right.join("solo-derecha.txt"), b"x").unwrap();
        let plan = plan_sync(
            &left,
            &right,
            SyncOptions::default(),
            &CancellationToken::new(),
        )
        .unwrap();
        assert!(plan
            .items
            .iter()
            .any(|item| item.relative == Path::new("nuevo.txt")
                && item.action == SyncAction::CopyLeftToRight));
        assert!(!plan
            .items
            .iter()
            .any(|item| item.action == SyncAction::DeleteRight));
        assert_eq!(plan.copy_bytes, 4);
    }

    #[test]
    fn borrado_de_sobrantes_es_explicito() {
        let tmp = tempfile::tempdir().unwrap();
        let left = tmp.path().join("left");
        let right = tmp.path().join("right");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::write(right.join("extra.txt"), b"x").unwrap();
        let plan = plan_sync(
            &left,
            &right,
            SyncOptions {
                mode: SyncMode::LeftToRight,
                delete_extras: true,
            },
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(plan.items[0].action, SyncAction::DeleteRight);
    }

    #[test]
    fn cancelacion_aborta_antes_de_recorrer() {
        let tmp = tempfile::tempdir().unwrap();
        let left = tmp.path().join("left");
        let right = tmp.path().join("right");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        let token = CancellationToken::new();
        token.cancel();
        assert_eq!(
            plan_sync(&left, &right, SyncOptions::default(), &token),
            Err(SyncError::Cancelled)
        );
    }

    #[test]
    fn bidireccional_no_adivina_si_ambos_lados_difieren_sin_fecha_concluyente() {
        let tmp = tempfile::tempdir().unwrap();
        let left = tmp.path().join("left");
        let right = tmp.path().join("right");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::write(left.join("dato.txt"), b"izquierda").unwrap();
        fs::write(right.join("dato.txt"), b"derecha-distinta").unwrap();
        let plan = plan_sync(
            &left,
            &right,
            SyncOptions {
                mode: SyncMode::Bidirectional,
                delete_extras: false,
            },
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].action, SyncAction::Conflict);
    }

    #[test]
    fn seleccion_parcial_genera_solo_los_pasos_marcados() {
        let tmp = tempfile::tempdir().unwrap();
        let left = tmp.path().join("left");
        let right = tmp.path().join("right");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::write(left.join("a.txt"), b"a").unwrap();
        fs::write(left.join("b.txt"), b"bb").unwrap();
        let plan = plan_sync(
            &left,
            &right,
            SyncOptions::default(),
            &CancellationToken::new(),
        )
        .unwrap();
        let selected: Vec<bool> = plan
            .items
            .iter()
            .map(|item| item.relative == Path::new("b.txt"))
            .collect();
        let op = plan.copy_op_plan(&selected);
        assert_eq!(op.steps.len(), 1);
        assert_eq!(op.steps[0].to, right.join("b.txt"));
        assert_eq!(op.total_bytes, 2);
        assert_eq!(op.total_files, 1);
    }

    #[test]
    fn origen_que_desaparece_tras_planificar_falla_sin_crear_destino() {
        let tmp = tempfile::tempdir().unwrap();
        let left = tmp.path().join("left");
        let right = tmp.path().join("right");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        let source = left.join("volatil.txt");
        fs::write(&source, b"contenido").unwrap();
        let sync = plan_sync(
            &left,
            &right,
            SyncOptions::default(),
            &CancellationToken::new(),
        )
        .unwrap();
        fs::remove_file(&source).unwrap();
        let op = sync.copy_op_plan(&[true]);
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_decision_tx, decision_rx) = mpsc::channel();
        let summary = run_plan(
            &op,
            &OpKind::Copy,
            ConflictPolicy::Overwrite,
            &CancellationToken::new(),
            &tx,
            &decision_rx,
            None,
        );
        assert_eq!(summary.count_failed(), 1);
        assert!(!right.join("volatil.txt").exists());
    }

    #[test]
    fn destino_creado_tras_planificar_vuelve_a_pedir_conflicto_y_se_puede_saltar() {
        let tmp = tempfile::tempdir().unwrap();
        let left = tmp.path().join("left");
        let right = tmp.path().join("right");
        fs::create_dir_all(&left).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::write(left.join("dato.txt"), b"origen").unwrap();
        let sync = plan_sync(
            &left,
            &right,
            SyncOptions::default(),
            &CancellationToken::new(),
        )
        .unwrap();
        fs::write(right.join("dato.txt"), b"creado-luego").unwrap();
        let op = sync.copy_op_plan(&[true]);
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (decision_tx, decision_rx) = mpsc::channel();
        decision_tx
            .send(ConflictDecision {
                action: ConflictAction::Skip,
                apply_all: false,
            })
            .unwrap();
        let summary = run_plan(
            &op,
            &OpKind::Copy,
            ConflictPolicy::Ask,
            &CancellationToken::new(),
            &tx,
            &decision_rx,
            None,
        );
        assert_eq!(summary.count_skipped(), 1);
        assert_eq!(fs::read(right.join("dato.txt")).unwrap(), b"creado-luego");
    }

    #[test]
    fn conflicto_archivo_carpeta_nunca_se_selecciona_como_copia() {
        let tmp = tempfile::tempdir().unwrap();
        let left = tmp.path().join("left");
        let right = tmp.path().join("right");
        fs::create_dir_all(left.join("mismo")).unwrap();
        fs::create_dir_all(&right).unwrap();
        fs::write(right.join("mismo"), b"archivo").unwrap();
        let plan = plan_sync(
            &left,
            &right,
            SyncOptions::default(),
            &CancellationToken::new(),
        )
        .unwrap();
        assert!(plan.items.iter().any(|item| {
            item.relative == Path::new("mismo") && item.action == SyncAction::Conflict
        }));
        assert!(plan
            .copy_op_plan(&vec![true; plan.items.len()])
            .steps
            .is_empty());
    }
}
