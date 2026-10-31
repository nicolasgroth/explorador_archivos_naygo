// Naygo — workers para enumerar unidades y expulsar dispositivos sin bloquear la UI.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::{EjectCompletion, EjectOutcome, WorkspaceCtrl};
use naygo_core::icon_kind::DriveKind;
use naygo_core::tree::DirTree;
use naygo_core::workspace::PaneId;
use naygo_platform::dir_watch::Waker;
use std::path::PathBuf;
use std::sync::mpsc::{self, TryRecvError};

impl WorkspaceCtrl {
    /// Pide un snapshot de dispositivos. Si ya hay uno en curso, coalesce el evento y ejecuta
    /// una sola enumeración adicional al completar el worker actual.
    pub fn request_drive_refresh(&mut self, waker: Waker) {
        if self.drive_refresh_rx.is_some() {
            self.drive_refresh_pending = true;
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.drive_refresh_rx = Some(rx);
        std::thread::spawn(move || {
            let drives = naygo_platform::drives::drives();
            let _ = tx.send(drives);
            waker();
        });
    }

    /// Aplica resultados ya completados, sin esperar al worker. `true` significa que la toolbar
    /// y los árboles deben repintarse.
    pub fn pump_drive_refresh(&mut self, waker: Waker) -> bool {
        let result = match self.drive_refresh_rx.as_ref() {
            Some(rx) => match rx.try_recv() {
                Ok(drives) => Some(Some(drives)),
                Err(TryRecvError::Empty) => return false,
                Err(TryRecvError::Disconnected) => Some(None),
            },
            None => return false,
        };
        self.drive_refresh_rx = None;
        let mut changed = false;
        match result {
            Some(Some(drives)) => {
                // Una respuesta vacía puede ser un error transitorio de Win32; nunca borra un
                // snapshot que ya permitía navegar y validar expulsiones de forma segura.
                if !drives.is_empty() && drives != self.drive_snapshot {
                    self.drive_snapshot = drives;
                    let roots = self.drive_roots();
                    for tree in self.trees.values_mut() {
                        tree.replace_drives(&roots);
                    }
                    changed = true;
                }
            }
            Some(None) => {
                crate::logging::log_line("worker de enumeración de unidades desconectado")
            }
            None => {}
        }
        if self.drive_refresh_pending {
            self.drive_refresh_pending = false;
            self.request_drive_refresh(waker);
        }
        changed
    }

    pub fn drive_roots(&self) -> Vec<(PathBuf, String, DriveKind)> {
        self.drive_snapshot
            .iter()
            .map(|drive| (drive.path.clone(), drive.label.clone(), drive.kind))
            .collect()
    }

    /// Construye raíces conocidas con el snapshot disponible; llamadas posteriores actualizan
    /// las unidades cuando responda el worker, preservando expansión y selección.
    pub fn build_tree(&self) -> DirTree {
        let folders: Vec<(PathBuf, String, String)> =
            naygo_platform::known_folders::known_folders()
                .into_iter()
                .map(|folder| (folder.path, folder.name, folder.icon.tree_tag().to_string()))
                .collect();
        DirTree::from_folders_and_drives(&folders, &self.drive_roots())
    }

    /// Revalida la raíz con una enumeración reciente y ejecuta el eject completo en worker. La
    /// comprobación no usa el snapshot de UI: una letra que se haya reasignado tras abrir el modal
    /// jamás debe validar contra la unidad anterior.
    pub fn start_eject_drive(&mut self, root: PathBuf, panes: Vec<PaneId>, waker: Waker) -> bool {
        if self.eject_rx.is_some() {
            return false;
        }
        let (tx, rx) = mpsc::channel();
        self.eject_rx = Some(rx);
        std::thread::spawn(move || {
            let removable = root_is_removable(&root, &naygo_platform::drives::drives());
            let outcome = if removable {
                match naygo_platform::eject::eject_drive(&root) {
                    Ok(()) => EjectOutcome::Ok,
                    Err(naygo_platform::eject::EjectError::InUse) => EjectOutcome::InUse,
                    Err(error) => EjectOutcome::Failed(error.to_string()),
                }
            } else {
                EjectOutcome::Failed("not a removable drive".into())
            };
            let _ = tx.send(EjectCompletion {
                root,
                panes,
                outcome,
            });
            waker();
        });
        true
    }

    pub fn pump_eject_drive(&mut self) -> Option<EjectCompletion> {
        let rx = self.eject_rx.as_ref()?;
        match rx.try_recv() {
            Ok(completion) => {
                self.eject_rx = None;
                Some(completion)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.eject_rx = None;
                crate::logging::log_line("worker de expulsión desconectado");
                None
            }
        }
    }
}

fn root_is_removable(root: &std::path::Path, drives: &[naygo_platform::drives::DriveInfo]) -> bool {
    drives
        .iter()
        .any(|drive| drive.path == root && drive.kind == DriveKind::Removable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive(path: &str, kind: DriveKind) -> naygo_platform::drives::DriveInfo {
        naygo_platform::drives::DriveInfo {
            path: PathBuf::from(path),
            label: path.into(),
            kind,
        }
    }

    #[test]
    fn eject_solo_acepta_raiz_removible_en_snapshot_fresco() {
        let drives = [
            drive("C:\\", DriveKind::Fixed),
            drive("E:\\", DriveKind::Removable),
            drive("Z:\\", DriveKind::Network),
        ];

        assert!(root_is_removable(std::path::Path::new("E:\\"), &drives));
        assert!(!root_is_removable(std::path::Path::new("C:\\"), &drives));
        assert!(!root_is_removable(std::path::Path::new("Z:\\"), &drives));
        assert!(!root_is_removable(std::path::Path::new("F:\\"), &drives));
        assert!(!root_is_removable(
            std::path::Path::new("E:\\folder"),
            &drives
        ));
    }
}
