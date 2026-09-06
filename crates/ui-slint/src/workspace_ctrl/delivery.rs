// Naygo — revisión de una entrega antes de publicar una carpeta o un ZIP.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use naygo_core::delivery::{DeliveryError, DeliveryPlan, Output, ReviewOptions, Structure};

#[derive(Default)]
pub struct DeliveryState {
    pub label: Option<String>,
    pub open: bool,
    pub sources: Vec<PathBuf>,
    pub staging: Vec<naygo_platform::drop_target::StagedDrop>,
    pub plan: Option<DeliveryPlan>,
    pub error: String,
    pub report: String,
    pub group_names: String,
    pub rx: Option<std::sync::mpsc::Receiver<Result<DeliveryPlan, DeliveryError>>>,
    token: Option<naygo_core::CancellationToken>,
}
impl Drop for DeliveryState {
    fn drop(&mut self) {
        if let Some(token) = &self.token {
            token.cancel();
        }
    }
}

impl WorkspaceCtrl {
    pub fn delivery_open(&mut self) -> bool {
        let sources = if self.basket_context {
            self.basket_action_paths()
        } else {
            self.selected_paths()
        };
        if sources.is_empty() {
            return false;
        }
        self.delivery = DeliveryState::default();
        self.delivery.open = true;
        self.delivery.sources = sources;
        self.delivery.group_names =
            naygo_core::delivery::suggested_group_names(&self.delivery.sources).join("\n");
        self.delivery.staging = self.basket_staging.clone();
        self.delivery.report = self
            .delivery
            .sources
            .iter()
            .take(500)
            .enumerate()
            .map(|(i, p)| format!("{}: {}\n", i + 1, p.display()))
            .collect();
        true
    }

    pub fn delivery_invalidate(&mut self) {
        if let Some(token) = self.delivery.token.take() {
            token.cancel();
        }
        self.delivery.rx = None;
        self.delivery.plan = None;
        self.delivery.error.clear();
        self.delivery.report = self
            .delivery
            .sources
            .iter()
            .take(500)
            .enumerate()
            .map(|(i, p)| format!("{}: {}\n", i + 1, p.display()))
            .collect();
    }

    pub fn delivery_close(&mut self) {
        self.delivery = DeliveryState::default();
    }

    pub fn delivery_prepare(
        &mut self,
        destination: PathBuf,
        mode: i32,
        root: PathBuf,
        zip: bool,
        hashes: bool,
        options: ReviewOptions,
    ) {
        self.delivery_invalidate();
        let structure = match mode {
            0 => Structure::Grouped,
            1 => Structure::Flat,
            _ => Structure::Relative(root),
        };
        let sources = self.delivery.sources.clone();
        let guards = self.delivery.staging.clone();
        let token = naygo_core::CancellationToken::new();
        let worker_token = token.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _guards = guards;
            let result = naygo_core::delivery::prepare_review(
                &sources,
                &destination,
                &structure,
                if zip { Output::Zip } else { Output::Folder },
                &options,
                &worker_token,
            )
            .and_then(|mut plan| {
                if hashes {
                    plan.capture_hashes(&worker_token)?;
                }
                Ok(plan)
            });
            let _ = tx.send(result);
        });
        self.delivery.token = Some(token);
        self.delivery.rx = Some(rx);
        self.delivery.report = self.config.t("meta.loading");
    }

    pub fn pump_delivery(&mut self) -> bool {
        self.pump_delivery_results();
        let Some(rx) = &self.delivery.rx else {
            return true;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err(DeliveryError::Invalid("delivery worker disconnected"))
            }
        };
        self.delivery.rx = None;
        self.delivery.token = None;
        match result {
            Ok(plan) => {
                self.delivery.report = format!(
                    "{}\n{} · {}\n\n{}{}",
                    plan.destination.display(),
                    plan.entries.len(),
                    naygo_core::format::human_size(plan.total_bytes),
                    plan.entries
                        .iter()
                        .take(500)
                        .map(|e| format!(
                            "{} → {}\t{}\n",
                            e.source().display(),
                            e.relative.display(),
                            naygo_core::format::human_size(e.size())
                        ))
                        .collect::<String>(),
                    if plan.entries.len() > 500 { "…" } else { "" }
                );
                self.delivery.plan = Some(plan);
            }
            Err(DeliveryError::Conflicts(conflicts)) => {
                self.delivery.report = format!(
                    "{} ({})\n\n{}{}",
                    self.config.t("delivery.conflicts"),
                    conflicts.len(),
                    conflicts
                        .iter()
                        .flat_map(|conflict| conflict.sources.iter().map(move |p| format!(
                            "{} → {}\n",
                            p.display(),
                            conflict.relative.display()
                        )))
                        .take(500)
                        .collect::<String>(),
                    if conflicts.iter().map(|c| c.sources.len()).sum::<usize>() > 500 {
                        "…"
                    } else {
                        ""
                    }
                );
                self.delivery.error = self.config.t("delivery.conflicts");
            }
            Err(error) => {
                self.delivery.error = error.to_string();
                self.delivery.report = self.delivery.error.clone();
            }
        }
        true
    }

    pub fn delivery_execute(&mut self) -> bool {
        // Respetar la cola: no arrancar un flujo compuesto junto a otro trabajo pesado.
        // Mantener la revisión abierta hasta que pueda ejecutarse, sin descartarla.
        if self.ops.ops_mode == crate::ops_ctrl::OpsMode::Queue && self.ops.any_running() {
            return false;
        }
        let Some(plan) = self.delivery.plan.take() else {
            return false;
        };
        let staging = std::mem::take(&mut self.delivery.staging);
        let output = plan.output;
        let id = self.ops.start_delivery(
            plan,
            self.delivery
                .label
                .clone()
                .unwrap_or_else(|| self.config.t("delivery.title")),
            staging.clone(),
        );
        self.delivery_results.pending.push(DeliveryJob {
            id,
            sources: self.delivery.sources.clone(),
            staging,
            output,
        });
        self.delivery_close();
        self.ensure_ops_pane();
        true
    }
}

pub struct DeliveryJob {
    id: u64,
    sources: Vec<PathBuf>,
    staging: Vec<naygo_platform::drop_target::StagedDrop>,
    output: Output,
}

pub struct DeliveryReceipt {
    pub destination: Option<PathBuf>,
    pub message: String,
    job: DeliveryJob,
}

#[derive(Default)]
pub struct DeliveryResults {
    pending: Vec<DeliveryJob>,
    pub latest: Option<DeliveryReceipt>,
}

impl WorkspaceCtrl {
    fn pump_delivery_results(&mut self) {
        let mut i = 0;
        while i < self.delivery_results.pending.len() {
            let id = self.delivery_results.pending[i].id;
            let Some(op) = self.ops.active_ops.iter().find(|op| op.id == id) else {
                self.delivery_results.pending.remove(i);
                continue;
            };
            let Some(summary) = &op.summary else {
                i += 1;
                continue;
            };
            let destination = summary
                .items
                .iter()
                .find(|item| matches!(item.outcome, naygo_core::ops::OpOutcome::Done))
                .map(|item| item.dest.clone());
            let message = if let Some(path) = &destination {
                format!(
                    "{}\n{}",
                    self.config.t("delivery.completed"),
                    path.display()
                )
            } else if op.token.is_cancelled() {
                self.config.t("app.cancelled")
            } else {
                let error = summary
                    .items
                    .iter()
                    .find_map(|item| match &item.outcome {
                        naygo_core::ops::OpOutcome::Failed(error) => Some(error.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");
                format!("{}\n{}", self.config.t("delivery.failed"), error)
            };
            let job = self.delivery_results.pending.remove(i);
            self.delivery_results.latest = Some(DeliveryReceipt {
                destination,
                message,
                job,
            });
        }
    }

    /// Recupera las fuentes capturadas como marcas en la bandeja, sin eliminar otras referencias.
    pub fn delivery_return_to_selection(&mut self) -> bool {
        let Some(receipt) = self.delivery_results.latest.take() else {
            return false;
        };
        self.basket.add(receipt.job.sources.iter().cloned());
        self.basket_staging.extend(receipt.job.staging);
        self.basket_selection = Default::default();
        let keys: std::collections::HashSet<_> = receipt
            .job
            .sources
            .iter()
            .map(|p| p.to_string_lossy().replace('/', "\\").to_lowercase())
            .collect();
        for (i, path) in self.basket.items().iter().enumerate() {
            if keys.contains(&path.to_string_lossy().replace('/', "\\").to_lowercase()) {
                self.basket_selection
                    .select(self.basket.items(), i, true, false);
            }
        }
        let basket = self.ws.layout.pane_ids().into_iter().find(|id| {
            self.ws
                .pane(*id)
                .is_some_and(|p| p.purpose == PanePurpose::Basket)
        });
        if let Some(id) = basket {
            self.set_active(id);
        } else {
            self.add_pane_of(PanePurpose::Basket, self.last_area);
        }
        self.basket_context = true;
        true
    }

    pub fn delivery_result_location(&self, containing_folder: bool) -> Option<PathBuf> {
        let receipt = self.delivery_results.latest.as_ref()?;
        let destination = receipt.destination.as_ref()?;
        if containing_folder && receipt.job.output == Output::Zip {
            destination.parent().map(Path::to_path_buf)
        } else {
            Some(destination.clone())
        }
    }
}
