// Naygo — editor de recetas y revisión congelada antes de usar el motor de entregas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
use naygo_core::{
    recipe::{self, Parameters, Prepared, Recipe, RecipeError, Source},
    saved_search::{RelativeDate, SavedSearch},
    task_space::SpaceError,
    CancellationToken,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
#[cfg(test)]
#[path = "recipes_tests.rs"]
mod tests;

enum JobResult {
    Loaded(Recipe, PathBuf, String),
    Query(SavedSearch),
    Prepared(Box<Prepared>),
}
pub struct RecipesState {
    pub open: bool,
    pub revision: i32,
    pub draft: Recipe,
    pub parameters: Parameters,
    pub report: String,
    pub references: Vec<PathBuf>,
    selected: Vec<PathBuf>,
    staging: Vec<naygo_platform::drop_target::StagedDrop>,
    loaded: Option<(PathBuf, String)>,
    plan: Option<Prepared>,
    rx: Option<Receiver<Result<JobResult, RecipeError>>>,
    token: Option<CancellationToken>,
    progress: Arc<AtomicUsize>,
}
impl Default for RecipesState {
    fn default() -> Self {
        Self {
            open: false,
            revision: 0,
            draft: Recipe::from_query(&SavedSearch {
                version: 1,
                name: String::new(),
                roots: vec![],
                options: Default::default(),
                min_bytes: None,
                max_bytes: None,
                modified: RelativeDate::Any,
            }),
            parameters: Parameters {
                roots: vec![],
                parent: PathBuf::new(),
            },
            report: String::new(),
            references: vec![],
            selected: vec![],
            staging: vec![],
            loaded: None,
            plan: None,
            rx: None,
            token: None,
            progress: Arc::new(AtomicUsize::new(0)),
        }
    }
}
impl Drop for RecipesState {
    fn drop(&mut self) {
        if let Some(t) = &self.token {
            t.cancel();
        }
    }
}
impl RecipesState {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
    pub fn ready(&self) -> bool {
        self.plan.is_some()
    }
    pub fn can_update(&self) -> bool {
        self.loaded.is_some()
    }
    pub fn progress(&self) -> usize {
        self.progress.load(Ordering::Relaxed)
    }
}
impl WorkspaceCtrl {
    pub fn recipe_open(&mut self) -> bool {
        if self.any_modal_open() {
            return false;
        }
        self.recipes.open = true;
        self.recipes.selected = if self.basket_context {
            self.basket_action_paths()
        } else {
            self.selected_paths()
        };
        self.recipes.staging = self.basket_staging.clone();
        self.recipes.plan = None;
        if self.recipes.draft.name.is_empty() {
            self.recipes.draft.name = self.config.t("recipes.default_name");
            self.recipes.draft.source = Source::Selection;
            self.recipes.parameters.roots = self.active_dir().into_iter().collect();
            self.recipes.parameters.parent = self.active_dir().unwrap_or_default();
        }
        self.recipes.report = format!(
            "{}: {}",
            self.config.t("recipes.selection"),
            self.recipes.selected.len()
        );
        self.recipes.revision = self.recipes.revision.wrapping_add(1);
        true
    }
    pub fn recipe_close(&mut self) {
        if let Some(token) = self.recipes.token.take() {
            token.cancel();
        }
        self.recipes.rx = None;
        self.recipes.plan = None;
        self.recipes.open = false;
        self.recipes.selected.clear();
        self.recipes.staging.clear();
    }
    pub fn recipe_invalidate(&mut self) {
        self.recipes.plan = None;
    }
    pub fn recipe_edit(&mut self, recipe: Recipe, parameters: Parameters) {
        if self.recipes.busy() {
            return;
        }
        if self.recipes.draft != recipe || self.recipes.parameters != parameters {
            self.recipe_invalidate();
        }
        self.recipes.draft = recipe;
        self.recipes.parameters = parameters;
    }
    pub fn recipe_invalid(&mut self) {
        self.recipe_invalidate();
        self.recipes.report = self.config.t("recipes.invalid");
    }
    fn recipe_job(
        &mut self,
        job: impl FnOnce(CancellationToken, Arc<AtomicUsize>) -> Result<JobResult, RecipeError>
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
        self.recipes.rx = Some(rx);
        self.recipes.token = Some(token);
        self.recipes.progress = progress;
        self.recipes.plan = None;
    }
    pub fn recipe_read(&mut self, path: PathBuf, query: bool) {
        if self.recipes.busy() {
            return;
        }
        self.recipe_job(move |token, _| {
            if query {
                return Ok(JobResult::Query(
                    naygo_core::saved_search::read(&path, &token)?.0,
                ));
            }
            let (recipe, rev) = recipe::read(&path, &token)?;
            Ok(JobResult::Loaded(recipe, path, rev))
        });
    }
    pub fn recipe_save(&mut self, path: Option<PathBuf>) {
        if self.recipes.busy() {
            return;
        }
        let (path, expected) = match (path, &self.recipes.loaded) {
            (Some(path), _) => (path, None),
            (None, Some((path, rev))) => (path.clone(), Some(rev.clone())),
            _ => return,
        };
        let recipe = self.recipes.draft.clone();
        self.recipe_job(move |token, _| {
            let revision =
                naygo_platform::recipe_store::save(&path, &recipe, expected.as_deref(), &token)?;
            Ok(JobResult::Loaded(recipe, path, revision))
        });
    }
    pub fn recipe_forget(&mut self) {
        if self.recipes.busy() {
            return;
        }
        if let Some((path, _)) = self.recipes.loaded.take() {
            self.recipes.references.retain(|p| p != &path);
        }
    }
    pub fn recipe_prepare(&mut self) {
        if self.recipes.busy() {
            return;
        }
        let recipe = self.recipes.draft.clone();
        let parameters = self.recipes.parameters.clone();
        let selected = self.recipes.selected.clone();
        let guards = self.recipes.staging.clone();
        self.recipe_job(move |token, count| {
            let _guards = guards;
            let time =
                naygo_platform::time::recipe_time(recipe.modified).ok_or(SpaceError::Invalid)?;
            let prepared = recipe::prepare(recipe, parameters, selected, time, &token, |n| {
                count.store(n, Ordering::Relaxed);
            })?;
            Ok(JobResult::Prepared(Box::new(prepared)))
        });
    }
    pub fn recipe_execute(&mut self) -> bool {
        if self.recipes.busy()
            || (self.ops.ops_mode == crate::ops_ctrl::OpsMode::Queue && self.ops.any_running())
        {
            return false;
        }
        let Some(prepared) = self.recipes.plan.take() else {
            return false;
        };
        if prepared.recipe != self.recipes.draft || prepared.parameters != self.recipes.parameters {
            self.recipe_invalid();
            return false;
        }
        self.delivery = super::delivery::DeliveryState::default();
        self.delivery.sources = prepared.sources;
        self.delivery.plan = Some(prepared.plan);
        self.delivery.staging = self.recipes.staging.clone();
        self.delivery.label = Some(format!(
            "{}: {}",
            self.config.t("recipes.run"),
            prepared.recipe.name
        ));
        self.recipe_close();
        self.delivery_execute()
    }
    pub fn pump_recipes(&mut self) -> bool {
        let Some(rx) = &self.recipes.rx else {
            return true;
        };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(_) => {
                Err(SpaceError::Io(std::io::Error::other("recipe worker disconnected")).into())
            }
        };
        self.recipes.rx = None;
        self.recipes.token = None;
        match result {
            Ok(JobResult::Loaded(recipe, path, rev)) => {
                self.recipes.draft = recipe;
                self.recipes.report =
                    format!("{}\n{}", path.display(), self.config.t("recipes.loaded"));
                if !self.recipes.references.contains(&path) {
                    self.recipes.references.push(path.clone());
                    if self.recipes.references.len() > 64 {
                        self.recipes.references.remove(0);
                    }
                }
                self.recipes.loaded = Some((path, rev));
            }
            Ok(JobResult::Query(query)) => {
                self.recipes.draft = Recipe::from_query(&query);
                self.recipes.parameters.roots = query.roots;
                self.recipes.loaded = None;
                self.recipes.report = self.config.t("recipes.loaded");
            }
            Ok(JobResult::Prepared(prepared)) => {
                self.recipes.report = format!(
                    "{}\n{}: {} → {}: {}\n{}\n{} · {}\n\n{}{}",
                    self.config.t("recipes.steps"),
                    self.config.t("recipes.examined"),
                    prepared.examined,
                    self.config.t("recipes.matched"),
                    prepared.sources.len(),
                    prepared.plan.destination.display(),
                    prepared.plan.entries.len(),
                    naygo_core::format::human_size(prepared.plan.total_bytes),
                    prepared
                        .plan
                        .entries
                        .iter()
                        .take(500)
                        .map(|e| format!("{} → {}\n", e.source().display(), e.relative.display()))
                        .collect::<String>(),
                    if prepared.plan.entries.len() > 500 {
                        self.config.t("recipes.rows_limit")
                    } else {
                        String::new()
                    }
                );
                self.recipes.plan = Some(*prepared);
            }
            Err(error) => {
                let key = match &error {
                    RecipeError::Partial => "recipes.partial",
                    RecipeError::Empty => "recipes.empty",
                    RecipeError::SelectionFilesOnly => "recipes.files_only",
                    RecipeError::Document(SpaceError::Exists) => "spaces.exists",
                    RecipeError::Document(SpaceError::Changed) => "spaces.changed",
                    RecipeError::Document(SpaceError::Cancelled)
                    | RecipeError::Delivery(naygo_core::delivery::DeliveryError::Cancelled) => {
                        "spaces.cancelled"
                    }
                    RecipeError::Document(SpaceError::Version) => "recipes.version",
                    _ => "recipes.invalid",
                };
                self.recipes.report = self.config.t(key);
                match error {
                    RecipeError::Delivery(e) => self.recipes.report.push_str(&format!("\n{e}")),
                    RecipeError::Document(SpaceError::Io(e)) => {
                        self.recipes.report.push_str(&format!("\n{e}"))
                    }
                    _ => {}
                }
            }
        }
        self.recipes.revision = self.recipes.revision.wrapping_add(1);
        true
    }
}
