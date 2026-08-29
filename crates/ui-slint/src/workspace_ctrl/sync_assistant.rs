// Naygo — asistente cancelable de sincronización entre dos paneles Files.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use crate::{SyncRowVm, SyncVm};
use naygo_core::sync_plan::{SyncAction, SyncError, SyncMode, SyncOptions, SyncPlan};
use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;

pub struct SyncAssistantState {
    pub left: PathBuf,
    pub right: PathBuf,
    pub options: SyncOptions,
    pub plan: Option<SyncPlan>,
    pub selected: Vec<bool>,
    pub error: Option<String>,
    pub rx: Option<std::sync::mpsc::Receiver<Result<SyncPlan, SyncError>>>,
    pub token: naygo_core::CancellationToken,
}

impl WorkspaceCtrl {
    pub fn sync_open(&mut self) -> bool {
        let Some(left_id) = self.active_files_id() else {
            return false;
        };
        let Some(right_id) = self.ws.other_files_panes(left_id).first().copied() else {
            self.pending_shell_error = Some(self.config.t("sync.need_two"));
            return false;
        };
        let Some(left) = self
            .ws
            .pane(left_id)
            .and_then(|pane| pane.files.as_ref())
            .map(|files| files.current_dir.clone())
        else {
            return false;
        };
        let Some(right) = self
            .ws
            .pane(right_id)
            .and_then(|pane| pane.files.as_ref())
            .map(|files| files.current_dir.clone())
        else {
            return false;
        };
        self.start_sync_worker(left, right, SyncOptions::default());
        true
    }

    fn start_sync_worker(&mut self, left: PathBuf, right: PathBuf, options: SyncOptions) {
        if let Some(old) = self.sync_assistant.take() {
            old.token.cancel();
        }
        let token = naygo_core::CancellationToken::new();
        let worker_token = token.clone();
        let worker_left = left.clone();
        let worker_right = right.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = naygo_core::sync_plan::plan_sync(
                &worker_left,
                &worker_right,
                options,
                &worker_token,
            );
            let _ = tx.send(result);
        });
        self.sync_assistant = Some(SyncAssistantState {
            left,
            right,
            options,
            plan: None,
            selected: Vec::new(),
            error: None,
            rx: Some(rx),
            token,
        });
    }

    pub fn sync_set_mode(&mut self, mode: i32) {
        let Some(state) = self.sync_assistant.as_ref() else {
            return;
        };
        let mode = match mode {
            1 => SyncMode::RightToLeft,
            2 => SyncMode::Bidirectional,
            _ => SyncMode::LeftToRight,
        };
        let left = state.left.clone();
        let right = state.right.clone();
        let delete_extras = state.options.delete_extras;
        self.start_sync_worker(
            left,
            right,
            SyncOptions {
                mode,
                delete_extras,
            },
        );
    }

    pub fn sync_set_delete_extras(&mut self, delete_extras: bool) {
        let Some(state) = self.sync_assistant.as_ref() else {
            return;
        };
        let left = state.left.clone();
        let right = state.right.clone();
        let mode = state.options.mode;
        self.start_sync_worker(
            left,
            right,
            SyncOptions {
                mode,
                delete_extras,
            },
        );
    }

    pub fn sync_toggle_item(&mut self, index: usize) {
        let Some(state) = self.sync_assistant.as_mut() else {
            return;
        };
        let is_conflict = state.plan.as_ref().is_some_and(|plan| {
            plan.items
                .get(index)
                .is_some_and(|item| item.action == SyncAction::Conflict)
        });
        if !is_conflict {
            if let Some(selected) = state.selected.get_mut(index) {
                *selected = !*selected;
            }
        }
    }

    pub fn sync_close(&mut self) {
        if let Some(state) = self.sync_assistant.take() {
            state.token.cancel();
        }
    }

    /// Drena el worker. Devuelve true si no queda planificación en vuelo.
    pub fn pump_sync(&mut self) -> bool {
        let Some(state) = self.sync_assistant.as_mut() else {
            return true;
        };
        let Some(rx) = state.rx.as_ref() else {
            return true;
        };
        match rx.try_recv() {
            Ok(Ok(plan)) => {
                state.selected = plan
                    .items
                    .iter()
                    .map(|item| item.action != SyncAction::Conflict)
                    .collect();
                state.plan = Some(plan);
                state.rx = None;
            }
            Ok(Err(error)) => {
                state.error = Some(match error {
                    SyncError::SameFolder => self.config.t("sync.same_folder"),
                    SyncError::Unreadable(path) => {
                        format!("{}: {}", self.config.t("sync.unreadable"), path.display())
                    }
                    SyncError::Cancelled => self.config.t("preview.err.cancelled"),
                });
                state.rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.error = Some(self.config.t("sync.unreadable"));
                state.rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
        }
        true
    }

    pub fn sync_apply(&mut self) -> bool {
        let Some(state) = self.sync_assistant.take() else {
            return false;
        };
        let Some(plan) = state.plan else {
            return false;
        };
        let copy_plan = plan.copy_op_plan(&state.selected);
        let delete_paths = plan.delete_paths(&state.selected);
        let mut started = false;
        if !copy_plan.steps.is_empty() {
            self.ensure_ops_pane();
            self.ops.start_preplanned(
                copy_plan,
                naygo_core::ops::OpKind::Copy,
                self.config.t("sync.operation"),
            );
            started = true;
        }
        if !delete_paths.is_empty() {
            self.ensure_ops_pane();
            self.ops.start_op(
                naygo_core::ops::delete(delete_paths, true),
                self.config.t("ops.file_kind_delete"),
                true,
            );
            started = true;
        }
        started
    }

    pub fn sync_vm(&self) -> SyncVm {
        let Some(state) = self.sync_assistant.as_ref() else {
            return SyncVm::default();
        };
        let rows = state
            .plan
            .as_ref()
            .map(|plan| {
                plan.items
                    .iter()
                    .enumerate()
                    .take(5000)
                    .map(|(index, item)| SyncRowVm {
                        path: SharedString::from(item.relative.to_string_lossy().as_ref()),
                        action: SharedString::from(match item.action {
                            SyncAction::CopyLeftToRight => self.config.t("sync.copy_lr"),
                            SyncAction::CopyRightToLeft => self.config.t("sync.copy_rl"),
                            SyncAction::DeleteLeft => self.config.t("sync.delete_left"),
                            SyncAction::DeleteRight => self.config.t("sync.delete_right"),
                            SyncAction::Conflict => self.config.t("sync.conflict"),
                        }),
                        selected: state.selected.get(index).copied().unwrap_or(false),
                        enabled: item.action != SyncAction::Conflict,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mode = match state.options.mode {
            SyncMode::LeftToRight => 0,
            SyncMode::RightToLeft => 1,
            SyncMode::Bidirectional => 2,
        };
        SyncVm {
            active: true,
            left: SharedString::from(state.left.to_string_lossy().as_ref()),
            right: SharedString::from(state.right.to_string_lossy().as_ref()),
            mode,
            delete_extras: state.options.delete_extras,
            planning: state.rx.is_some(),
            error: SharedString::from(state.error.as_deref().unwrap_or_default()),
            rows: ModelRc::from(Rc::new(VecModel::from(rows))),
            can_apply: state.plan.is_some() && state.selected.iter().any(|selected| *selected),
        }
    }
}
