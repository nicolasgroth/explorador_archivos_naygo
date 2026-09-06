// Naygo — asistente de reintento: revisar, confirmar y revalidar en workers.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use naygo_core::{
    ops::{
        retry::{self, RetryReview},
        OpKind,
    },
    CancellationToken,
};

#[derive(Default)]
pub struct RetryState {
    pub open: bool,
    pub report: String,
    pub kind: Option<OpKind>,
    review: Option<RetryReview>,
    rx: Option<Receiver<Result<RetryReview, std::io::Error>>>,
    token: Option<CancellationToken>,
    confirming: bool,
}
impl RetryState {
    fn new(kind: OpKind) -> Self {
        Self {
            open: true,
            report: String::new(),
            kind: Some(kind),
            review: None,
            rx: None,
            token: None,
            confirming: false,
        }
    }
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
    pub fn ready(&self) -> bool {
        self.review.is_some() && !self.busy()
    }
}
impl Drop for RetryState {
    fn drop(&mut self) {
        if let Some(token) = &self.token {
            token.cancel();
        }
    }
}

impl WorkspaceCtrl {
    pub fn retry_open(&mut self, id: u64) {
        if self.any_modal_open() {
            return;
        }
        let Some((kind, files, total_failed)) = self.ops.retry_files(id) else {
            return;
        };
        let kind_label = self.config.t(if kind == OpKind::Move {
            "ops.file_kind_move"
        } else {
            "ops.file_kind_copy"
        });
        self.retry = RetryState::new(kind);
        self.retry.report = format!(
            "{} · {}: {} / {}\n\n{}",
            kind_label,
            self.config.t("retry.title"),
            files.len(),
            total_failed,
            files
                .iter()
                .map(|f| format!("{}\n  → {}", f.source.display(), f.destination.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );
        self.retry_worker(files);
    }
    fn retry_worker(&mut self, files: Vec<retry::RetryFile>) {
        let token = CancellationToken::new();
        let worker = token.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(retry::review(files, &worker));
        });
        self.retry.token = Some(token);
        self.retry.rx = Some(rx);
    }
    pub fn retry_close(&mut self) {
        self.retry = Default::default();
    }
    pub fn retry_confirm(&mut self) {
        if !self.retry.open || !self.retry.ready() {
            return;
        }
        let files = self.retry.review.as_ref().unwrap().files.clone();
        self.retry.confirming = true;
        self.retry_worker(files);
    }
    pub fn pump_retry(&mut self) -> bool {
        let Some(rx) = &self.retry.rx else {
            return true;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(_) => Err(std::io::ErrorKind::BrokenPipe.into()),
        };
        self.retry.rx = None;
        self.retry.token = None;
        match result {
            Ok(review) if self.retry.confirming => {
                if self.retry.review.as_ref() == Some(&review) {
                    if let Some(kind) = self.retry.kind.clone() {
                        let label = self.config.t("retry.title");
                        self.ops.start_preplanned(review.plan(), kind, label);
                    }
                    self.retry_close();
                    // pump_ops ya pasó en este tick. Mantener el timer una vuelta más
                    // para que observe el motor recién iniciado (o su conflicto).
                    return false;
                } else {
                    self.retry.review = None;
                    self.retry
                        .report
                        .push_str(&format!("\n\n{}", self.config.t("retry.changed")));
                }
            }
            Ok(review) => {
                self.retry.review = Some(review);
            }
            Err(error) => {
                self.retry.review = None;
                self.retry.report.push_str(&format!(
                    "\n\n{}\n{}",
                    self.config.t("retry.invalid"),
                    error
                ));
            }
        }
        self.retry.confirming = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn drain(c: &mut WorkspaceCtrl) {
        for _ in 0..5000 {
            if c.pump_retry() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("retry worker timeout");
    }
    #[test]
    fn confirmation_revalidates_and_cancel_never_executes() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("dest");
        std::fs::write(&source, b"before").unwrap();
        let mut c = WorkspaceCtrl::new_in(dir.path().join("cfg"), dir.path().to_path_buf());
        c.retry = RetryState::new(OpKind::Copy);
        c.retry_worker(vec![retry::RetryFile {
            source: source.clone(),
            destination: destination.clone(),
        }]);
        drain(&mut c);
        assert!(c.retry.ready());
        assert!(!destination.exists());
        std::fs::write(&source, b"changed-content").unwrap();
        c.retry_confirm();
        drain(&mut c);
        assert!(!c.retry.ready());
        assert!(c.ops.active_ops.is_empty());
        c.retry_close();
        c.retry = RetryState::new(OpKind::Copy);
        c.retry_worker(vec![retry::RetryFile {
            source,
            destination: destination.clone(),
        }]);
        c.retry_close();
        drain(&mut c);
        assert!(c.ops.active_ops.is_empty());
        assert!(!destination.exists());
    }
    #[test]
    fn confirmed_retry_uses_exact_path_and_keeps_other_files() {
        for kind in [OpKind::Copy, OpKind::Move] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("source");
            let destination = dir.path().join("nested/failed");
            let untouched = dir.path().join("already-done");
            std::fs::write(&source, b"retry").unwrap();
            std::fs::write(&untouched, b"keep").unwrap();
            let mut c = WorkspaceCtrl::new_in(dir.path().join("cfg"), dir.path().to_path_buf());
            c.retry = RetryState::new(kind.clone());
            c.retry_worker(vec![retry::RetryFile {
                source: source.clone(),
                destination: destination.clone(),
            }]);
            drain(&mut c);
            c.retry_confirm();
            drain(&mut c);
            assert!(!c.retry.open);
            assert_eq!(c.ops.active_ops.len(), 1);
            for _ in 0..5000 {
                c.ops.pump_ops();
                if c.ops.active_ops[0].summary.is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            assert_eq!(std::fs::read(destination).unwrap(), b"retry");
            assert_eq!(std::fs::read(untouched).unwrap(), b"keep");
            assert_eq!(source.exists(), kind == OpKind::Copy);
        }
    }
}
