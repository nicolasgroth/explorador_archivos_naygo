// Naygo — gestor de puntos: toda captura, lectura y publicación vive en workers.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
#[cfg(test)]
#[path = "comparison_points_tests.rs"]
mod tests;
use naygo_core::{
    comparison_point::{self as point, ChangeKind, Comparison, ComparisonPoint, Scope},
    task_space::SpaceError,
    CancellationToken,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone)]
struct Loaded {
    path: PathBuf,
    revision: String,
    point: Arc<ComparisonPoint>,
}
enum ResultKind {
    Loaded(Loaded),
    Compared(Comparison),
    Exported(PathBuf),
    Recycled,
}
#[derive(Default)]
pub struct ComparisonPointsState {
    pub open: bool,
    pub revision: i32,
    pub root: String,
    pub exclusions: String,
    pub recursive: bool,
    pub hashed: bool,
    pub report: String,
    pub confirm_delete: bool,
    pub rows: Vec<crate::PointRowVm>,
    paths: Vec<Option<PathBuf>>,
    loaded: Option<Loaded>,
    rx: Option<Receiver<Result<ResultKind, SpaceError>>>,
    token: Option<CancellationToken>,
    progress: Arc<AtomicUsize>,
}
impl Drop for ComparisonPointsState {
    fn drop(&mut self) {
        if let Some(t) = &self.token {
            t.cancel();
        }
    }
}
impl ComparisonPointsState {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
    pub fn loaded(&self) -> bool {
        self.loaded.is_some()
    }
    pub fn progress(&self) -> usize {
        self.progress.load(Ordering::Relaxed)
    }
    pub fn selected(&self) -> bool {
        self.rows.iter().any(|r| r.selected)
    }
}
impl WorkspaceCtrl {
    pub fn points_open(&mut self) -> bool {
        if self.any_modal_open() {
            return false;
        }
        if self.comparison_points.root.is_empty() {
            self.comparison_points.root = self
                .active_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            self.comparison_points.recursive = true;
        }
        self.comparison_points.open = true;
        self.comparison_points.revision = self.comparison_points.revision.wrapping_add(1);
        true
    }
    pub fn points_close(&mut self) {
        if self.comparison_points.confirm_delete {
            self.comparison_points.confirm_delete = false;
            return;
        }
        if let Some(t) = &self.comparison_points.token {
            t.cancel();
        }
        self.comparison_points.open = false;
    }
    pub fn points_new(&mut self) {
        if self.comparison_points.busy() {
            return;
        }
        let revision = self.comparison_points.revision.wrapping_add(1);
        self.comparison_points = ComparisonPointsState::default();
        self.comparison_points.open = true;
        self.comparison_points.revision = revision;
        self.comparison_points.recursive = true;
        self.comparison_points.root = self
            .active_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
    }
    fn points_error(&mut self, error: SpaceError) {
        let key = match error {
            SpaceError::Version => "points.version",
            SpaceError::Limit => "points.limit",
            SpaceError::Changed => "points.changed",
            SpaceError::Exists => "spaces.exists",
            SpaceError::Cancelled => "spaces.cancelled",
            SpaceError::Io(_) => "spaces.io",
            _ => "points.invalid",
        };
        self.comparison_points.report = self.config.t(key);
        if let SpaceError::Io(e) = error {
            self.comparison_points.report.push_str(&format!("\n{e}"));
        }
    }
    fn points_job(
        &mut self,
        job: impl FnOnce(CancellationToken, Arc<AtomicUsize>) -> Result<ResultKind, SpaceError>
            + Send
            + 'static,
    ) {
        let token = CancellationToken::new();
        let worker = token.clone();
        let progress = Arc::new(AtomicUsize::new(0));
        let count = progress.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(job(worker, count));
        });
        self.comparison_points.rx = Some(rx);
        self.comparison_points.token = Some(token);
        self.comparison_points.progress = progress;
        self.comparison_points.confirm_delete = false;
    }
    pub fn points_capture(&mut self, path: PathBuf, mut scope: Scope) {
        if self.comparison_points.busy() || self.comparison_points.loaded() {
            return;
        }
        self.comparison_points.root = scope.root.display().to_string();
        self.comparison_points.recursive = scope.recursive;
        self.comparison_points.hashed = scope.hashed;
        self.comparison_points.exclusions = scope
            .exclusions
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        if let Err(e) = scope.exclude_document(&path) {
            self.points_error(e);
            return;
        }
        self.points_job(move |token, progress| {
            let point = point::capture(scope, &token, |n| {
                progress.store(n, Ordering::Relaxed);
            })?;
            let revision = naygo_platform::comparison_point_store::save(&path, &point, &token)?;
            Ok(ResultKind::Loaded(Loaded {
                path,
                revision,
                point: Arc::new(point),
            }))
        });
    }
    pub fn points_read(&mut self, path: PathBuf) {
        if self.comparison_points.busy() {
            return;
        }
        self.points_job(move |token, _| {
            let (point, revision) = point::read(&path, &token)?;
            Ok(ResultKind::Loaded(Loaded {
                path,
                revision,
                point: Arc::new(point),
            }))
        });
    }
    pub fn points_compare(&mut self) {
        if self.comparison_points.busy() {
            return;
        }
        let Some(loaded) = self.comparison_points.loaded.clone() else {
            return;
        };
        // Nunca ofrecer la selección de una comparación anterior mientras se recalcula.
        self.comparison_points.rows.clear();
        self.comparison_points.paths.clear();
        self.comparison_points.revision = self.comparison_points.revision.wrapping_add(1);
        self.points_job(move |token, progress| {
            let current = point::capture(loaded.point.scope.clone(), &token, |n| {
                progress.store(n, Ordering::Relaxed);
            })?;
            Ok(ResultKind::Compared(point::compare(
                &loaded.point,
                &current,
                &token,
            )?))
        });
    }
    pub fn points_export(&mut self, path: PathBuf) {
        if self.comparison_points.busy() {
            return;
        }
        let Some(loaded) = self.comparison_points.loaded.clone() else {
            return;
        };
        self.points_job(move |token, _| {
            naygo_platform::comparison_point_store::save(&path, &loaded.point, &token)?;
            Ok(ResultKind::Exported(path))
        });
    }
    pub fn points_delete(&mut self) {
        if self.comparison_points.busy() {
            return;
        }
        let Some(loaded) = self.comparison_points.loaded.clone() else {
            return;
        };
        if !self.comparison_points.confirm_delete {
            self.comparison_points.confirm_delete = true;
            // La última exportación puede mostrar otro destino; confirmar siempre el ORIGINAL.
            self.comparison_points.report = format!(
                "{}\n{}",
                loaded.path.display(),
                self.config.t("points.delete_confirm")
            );
            return;
        }
        self.points_job(move |token, _| {
            naygo_platform::comparison_point_store::recycle(
                &loaded.path,
                &loaded.revision,
                &token,
            )?;
            Ok(ResultKind::Recycled)
        });
    }
    pub fn points_select(&mut self, index: i32, selected: bool) {
        if self.comparison_points.busy() {
            return;
        }
        if let Some(row) = usize::try_from(index)
            .ok()
            .and_then(|i| self.comparison_points.rows.get_mut(i))
        {
            row.selected = selected && row.actionable;
            self.comparison_points.revision = self.comparison_points.revision.wrapping_add(1);
        }
    }
    pub fn points_to_basket(&mut self) {
        if self.comparison_points.busy() {
            return;
        }
        let paths: Vec<_> = self
            .comparison_points
            .rows
            .iter()
            .zip(&self.comparison_points.paths)
            .filter(|(r, _)| r.selected && r.actionable)
            .filter_map(|(_, p)| p.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        self.basket.add(paths);
        self.comparison_points.open = false;
        if let Some(id) = self
            .ws
            .panes()
            .iter()
            .find(|p| p.purpose == PanePurpose::Basket)
            .map(|p| p.id)
        {
            self.set_active(id);
        } else {
            self.add_pane_of(PanePurpose::Basket, self.last_area);
        }
    }
    pub fn pump_comparison_points(&mut self) -> bool {
        let Some(rx) = self.comparison_points.rx.as_ref() else {
            return true;
        };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(_) => Err(SpaceError::Io(std::io::Error::other(
                "comparison worker disconnected",
            ))),
        };
        self.comparison_points.rx = None;
        self.comparison_points.token = None;
        match result {
            Err(e) => self.points_error(e),
            Ok(ResultKind::Loaded(loaded)) => {
                let p = &loaded.point;
                self.comparison_points.root = p.scope.root.display().to_string();
                self.comparison_points.exclusions = p
                    .scope
                    .exclusions
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                self.comparison_points.recursive = p.scope.recursive;
                self.comparison_points.hashed = p.scope.hashed;
                let date = naygo_core::format::format_time(
                    Some(p.captured_at as i64),
                    naygo_core::format::DateFormat::IsoMinute,
                );
                self.comparison_points.report = format!(
                    "{}\n{} · {}\n{}: {}\n{}",
                    loaded.path.display(),
                    self.config.t("points.captured"),
                    date,
                    self.config.t("points.entries"),
                    p.entries.len(),
                    self.config.t(if p.complete() {
                        "points.complete"
                    } else {
                        "points.partial"
                    })
                );
                self.comparison_points.rows.clear();
                self.comparison_points.paths.clear();
                self.comparison_points.loaded = Some(loaded);
            }
            Ok(ResultKind::Compared(report)) => {
                let kinds = [
                    ChangeKind::Added,
                    ChangeKind::Modified,
                    ChangeKind::Missing,
                    ChangeKind::Unknown,
                    ChangeKind::SameMetadata,
                    ChangeKind::SameContent,
                ];
                let mut summary = kinds
                    .iter()
                    .zip(report.counts)
                    .map(|(kind, n)| format!("{}: {n}", self.config.t(kind.key())))
                    .collect::<Vec<_>>()
                    .join(" · ");
                summary.push_str(&format!(
                    "\n{}",
                    self.config.t(if report.partial {
                        "points.partial"
                    } else {
                        "points.complete"
                    })
                ));
                if report.rows_truncated {
                    summary.push_str(&format!("\n{}", self.config.t("points.rows_limit")));
                }
                if let Some(loaded) = &self.comparison_points.loaded {
                    summary = format!("{}\n{}", loaded.path.display(), summary);
                    self.comparison_points.paths = report
                        .rows
                        .iter()
                        .map(|r| r.actionable.then(|| loaded.point.scope.root.join(&r.path)))
                        .collect();
                }
                self.comparison_points.rows = report
                    .rows
                    .iter()
                    .map(|r| crate::PointRowVm {
                        path: r.path.display().to_string().into(),
                        status: self.config.t(r.kind.key()).into(),
                        actionable: r.actionable,
                        selected: false,
                    })
                    .collect();
                self.comparison_points.report = summary;
            }
            Ok(ResultKind::Exported(path)) => {
                self.comparison_points.report =
                    format!("{}\n{}", self.config.t("points.exported"), path.display());
            }
            Ok(ResultKind::Recycled) => {
                let open = self.comparison_points.open;
                self.points_new();
                self.comparison_points.open = open;
                self.comparison_points.report = self.config.t("points.recycled");
            }
        }
        self.comparison_points.revision = self.comparison_points.revision.wrapping_add(1);
        true
    }
}
