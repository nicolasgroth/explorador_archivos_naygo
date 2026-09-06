// Naygo — consultas multiraíz guardadas, carga explícita y contexto de resultados.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
#[cfg(test)]
#[path = "saved_queries_tests.rs"]
mod tests;
use naygo_core::saved_search::{self, RelativeDate, SavedSearch};
use naygo_core::task_space::SpaceError;
use naygo_core::{basket::BasketSelection, search::SearchOptions, CancellationToken};

#[derive(Default)]
pub struct SearchDetails {
    pub advanced: Option<SavedSearch>,
    pub executed_at: Option<i64>,
    pub coverage: (usize, usize, bool),
    pub selection: BasketSelection,
}

struct Loaded {
    path: PathBuf,
    revision: String,
    query: SavedSearch,
    writing: bool,
}
pub struct SavedQueriesState {
    pub revision: i32,
    pub open: bool,
    pub draft: SavedSearch,
    pub report: String,
    pub references: Vec<PathBuf>,
    loaded: Option<(PathBuf, String)>,
    rx: Option<Receiver<Result<Loaded, SpaceError>>>,
    token: Option<CancellationToken>,
    accept_read: bool,
}
impl Default for SavedQueriesState {
    fn default() -> Self {
        Self {
            revision: 0,
            open: false,
            draft: SavedSearch {
                version: saved_search::VERSION,
                name: String::new(),
                roots: vec![],
                options: Default::default(),
                min_bytes: None,
                max_bytes: None,
                modified: RelativeDate::Any,
            },
            report: String::new(),
            references: vec![],
            loaded: None,
            rx: None,
            token: None,
            accept_read: false,
        }
    }
}
impl Drop for SavedQueriesState {
    fn drop(&mut self) {
        if let Some(token) = &self.token {
            token.cancel();
        }
    }
}
impl SavedQueriesState {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
    pub fn can_update(&self) -> bool {
        self.loaded.is_some()
    }
}

impl WorkspaceCtrl {
    pub fn query_open(&mut self, form: Option<(SearchOptions, String)>) -> bool {
        if self.any_modal_open() {
            return false;
        }
        if !self.saved_queries.busy() {
            if let Some((options, root)) = form {
                let roots = root
                    .lines()
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| PathBuf::from(s.trim()))
                    .collect();
                self.saved_queries.draft = SavedSearch {
                    roots,
                    options,
                    name: self.config.t("queries.default_name"),
                    ..SavedQueriesState::default().draft
                };
                self.saved_queries.loaded = None;
                self.saved_queries.report.clear();
            } else if self.saved_queries.draft.roots.is_empty() {
                self.saved_queries.draft.name = self.config.t("queries.default_name");
                self.saved_queries.draft.roots = self.active_dir().into_iter().collect();
                self.saved_queries.draft.options = self.search_options();
            }
        }
        self.saved_queries.open = true;
        self.saved_queries.revision = self.saved_queries.revision.wrapping_add(1);
        true
    }
    pub fn query_close(&mut self) {
        self.saved_queries.open = false;
        self.saved_queries.accept_read = false;
        if let Some(token) = &self.saved_queries.token {
            token.cancel();
        }
    }
    fn query_error(&mut self, e: SpaceError) {
        let key = match e {
            SpaceError::Version => "queries.version",
            SpaceError::Limit => "queries.limit",
            SpaceError::Changed => "spaces.changed",
            SpaceError::Exists => "spaces.exists",
            SpaceError::Cancelled => "spaces.cancelled",
            SpaceError::Io(_) => "spaces.io",
            _ => "queries.invalid",
        };
        self.saved_queries.report = self.config.t(key);
        if let SpaceError::Io(e) = e {
            self.saved_queries.report.push_str(&format!("\n{e}"));
        }
    }
    pub fn query_read(&mut self, path: PathBuf) {
        if self.saved_queries.busy() {
            return;
        }
        self.saved_queries.accept_read = true;
        let token = CancellationToken::new();
        let worker = token.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = saved_search::read(&path, &worker).map(|(query, revision)| Loaded {
                path,
                query,
                revision,
                writing: false,
            });
            let _ = tx.send(result);
        });
        self.saved_queries.rx = Some(rx);
        self.saved_queries.token = Some(token);
    }
    pub fn query_save(&mut self, new_path: Option<PathBuf>, query: SavedSearch) {
        if self.saved_queries.busy() {
            return;
        }
        if let Err(e) = query.validate() {
            self.query_error(e);
            return;
        }
        let (path, expected) = if let Some(path) = new_path {
            (path, None)
        } else if let Some((path, rev)) = &self.saved_queries.loaded {
            (path.clone(), Some(rev.clone()))
        } else {
            return;
        };
        self.saved_queries.draft = query.clone();
        let token = CancellationToken::new();
        let worker = token.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = naygo_platform::saved_search_store::save(
                &path,
                &query,
                expected.as_deref(),
                &worker,
            )
            .map(|revision| Loaded {
                path,
                query,
                revision,
                writing: true,
            });
            let _ = tx.send(result);
        });
        self.saved_queries.rx = Some(rx);
        self.saved_queries.token = Some(token);
    }
    pub fn pump_saved_queries(&mut self) -> bool {
        let Some(rx) = &self.saved_queries.rx else {
            return true;
        };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(_) => Err(SpaceError::Invalid),
        };
        self.saved_queries.rx = None;
        self.saved_queries.token = None;
        match result {
            Ok(loaded)
                if loaded.writing
                    || (self.saved_queries.open && self.saved_queries.accept_read) =>
            {
                if !self.saved_queries.references.contains(&loaded.path)
                    && self.saved_queries.references.len() < 64
                {
                    self.saved_queries.references.push(loaded.path.clone());
                }
                self.saved_queries.report = format!(
                    "{}\n{}",
                    self.config.t(if loaded.writing {
                        "queries.saved"
                    } else {
                        "queries.loaded"
                    }),
                    loaded.path.display()
                );
                self.saved_queries.draft = loaded.query;
                self.saved_queries.loaded = Some((loaded.path, loaded.revision));
                self.saved_queries.revision = self.saved_queries.revision.wrapping_add(1);
            }
            Ok(_) => {}
            Err(e) => self.query_error(e),
        }
        true
    }
    pub fn query_execute(&mut self, query: SavedSearch) {
        if let Err(e) = query.validate() {
            self.query_error(e);
            return;
        }
        self.close_search();
        let token = CancellationToken::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let (tx, rx) = std::sync::mpsc::channel();
        let worker_query = query.clone();
        let worker = token.clone();
        std::thread::spawn(move || {
            let lower = naygo_platform::time::search_date_lower_bound(worker_query.modified, now);
            if worker_query.modified != RelativeDate::Any && lower.is_none() {
                let _ = tx.send(naygo_core::search::SearchMsg::Coverage {
                    unreadable: 1,
                    content_skipped: 0,
                    directory_cap: false,
                });
                let _ = tx.send(naygo_core::search::SearchMsg::Done {
                    partial: true,
                    hit_cap: false,
                });
            } else {
                naygo_core::search_many::run(&worker_query, now, lower, &worker, &tx);
            }
        });
        self.search_job = Some(SearchJob {
            root: query.roots[0].clone(),
            query: query.options.name_query.clone(),
            options: query.options.clone(),
            rx,
            token,
            hits: vec![],
            dirs_scanned: 0,
            done: false,
            cancelled: false,
            partial: false,
            hit_cap: false,
            details: SearchDetails {
                advanced: Some(query.clone()),
                executed_at: Some(now),
                ..Default::default()
            },
        });
        self.saved_queries.draft = query;
        self.saved_queries.open = false;
        self.search_context = true;
        self.basket_context = false;
        self.open_search_pane(self.last_area);
    }
    pub fn query_rerun(&mut self) {
        if let Some(query) = self
            .search_job
            .as_ref()
            .and_then(|j| j.details.advanced.clone())
        {
            let draft = self.saved_queries.draft.clone();
            self.query_execute(query);
            self.saved_queries.draft = draft;
        }
    }
    pub fn query_forget(&mut self) {
        if let Some((path, _)) = self.saved_queries.loaded.take() {
            self.saved_queries.references.retain(|p| p != &path);
            self.saved_queries.report = self.config.t("queries.forgotten");
        }
    }
    pub fn search_select(&mut self, index: usize, control: bool, shift: bool) -> bool {
        let pane = self
            .ws
            .panes()
            .iter()
            .find(|p| p.purpose == PanePurpose::Search)
            .map(|p| p.id);
        if let Some(pane) = pane {
            self.set_active(pane);
        }
        let Some(job) = self.search_job.as_mut() else {
            return false;
        };
        let paths: Vec<_> = job.hits.iter().map(|h| h.entry.path.clone()).collect();
        if !job.details.selection.select(&paths, index, control, shift) {
            return false;
        }
        self.search_context = true;
        self.basket_context = false;
        true
    }
    pub fn search_selected_entry(&self) -> Option<&naygo_core::fs_model::Entry> {
        if !self.search_context {
            return None;
        }
        let job = self.search_job.as_ref()?;
        let path = job.details.selection.focused.as_ref()?;
        job.hits
            .iter()
            .find(|h| &h.entry.path == path)
            .map(|h| &h.entry)
    }
    pub fn search_to_basket(&mut self) {
        let paths: Vec<_> = self
            .search_job
            .as_ref()
            .map(|j| {
                j.hits
                    .iter()
                    .filter(|h| j.details.selection.contains(&h.entry.path))
                    .map(|h| h.entry.path.clone())
                    .collect()
            })
            .unwrap_or_default();
        self.basket.add(paths);
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
    pub fn search_focused_index(&self) -> i32 {
        self.search_job
            .as_ref()
            .and_then(|j| {
                j.hits
                    .iter()
                    .position(|h| Some(&h.entry.path) == j.details.selection.focused.as_ref())
            })
            .map_or(-1, |i| i as i32)
    }
    pub fn search_marked_count(&self) -> usize {
        self.search_job
            .as_ref()
            .map_or(0, |j| j.details.selection.count())
    }
    pub fn search_handle_action(&mut self, action: Action) -> bool {
        let count = self.search_job.as_ref().map_or(0, |j| j.hits.len());
        let current = self.search_focused_index();
        let index = match action {
            Action::MoveUp | Action::ExtendUp => (current - 1).max(0),
            Action::MoveDown | Action::ExtendDown => current + 1,
            Action::FocusHome | Action::ExtendHome => 0,
            Action::FocusEnd | Action::ExtendEnd => count as i32 - 1,
            Action::FocusPageUp | Action::ExtendPageUp => (current - 20).max(0),
            Action::FocusPageDown | Action::ExtendPageDown => current + 20,
            Action::Activate | Action::Open => {
                if current >= 0 {
                    self.open_search_hit(current as usize);
                }
                return true;
            }
            Action::ToggleFocused | Action::ToggleSelect => {
                if current >= 0 {
                    self.search_select(current as usize, true, false);
                }
                return true;
            }
            Action::SelectAll => {
                if count > 0 {
                    self.search_select(0, false, false);
                    self.search_select(count - 1, false, true);
                }
                return true;
            }
            _ => return true, // Nunca operar por accidente sobre el Files anterior.
        };
        if count > 0 {
            self.search_select(
                index.clamp(0, count as i32 - 1) as usize,
                false,
                matches!(
                    action,
                    Action::ExtendUp
                        | Action::ExtendDown
                        | Action::ExtendHome
                        | Action::ExtendEnd
                        | Action::ExtendPageUp
                        | Action::ExtendPageDown
                ),
            );
        }
        true
    }
}
