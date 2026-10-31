// Naygo — espacios por tarea: snapshots explícitos y persistencia en workers.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use naygo_core::task_space::{self, SpaceError, TaskSpace};
use naygo_core::CancellationToken;
use std::time::Instant;

#[cfg(test)]
#[path = "task_spaces_tests.rs"]
mod tests;

#[derive(Clone)]
struct SavedSpace {
    path: PathBuf,
    revision: String,
    stored: TaskSpace,
    resolved: TaskSpace,
}
enum Completed {
    Read(Box<SavedSpace>),
    Written(Box<SavedSpace>, bool),
}
#[derive(Default)]
pub struct TaskSpacesState {
    // MRU acotado: guarda referencias explícitas en settings.json; nunca explora carpetas.
    pub recents: Vec<PathBuf>,
    pub open: bool,
    pub report: String,
    pub dirty: bool,
    /// Un acceso desde la lista rápida aplica al terminar la lectura solo si el espacio actual
    /// sigue limpio. Si hay cambios, el documento queda en revisión dentro del gestor.
    quick_apply_if_clean: bool,
    recents_cleared: bool,
    /// Copia en memoria del árbol global para alternar a espacios que traen Favoritos propios.
    pub global_favorites: naygo_core::favorites::Favorites,
    /// Preferencia temporal del control «Guardar como»; no se persiste hasta crear el espacio.
    pub create_private_favorites: bool,
    active: Option<SavedSpace>,
    pending: Option<SavedSpace>,
    rx: Option<Receiver<Result<Completed, SpaceError>>>,
    token: Option<CancellationToken>,
    accept_read: bool,
}
impl Drop for TaskSpacesState {
    fn drop(&mut self) {
        if let Some(token) = &self.token {
            token.cancel();
        }
    }
}
impl TaskSpacesState {
    fn remember(&mut self, path: PathBuf) {
        if !valid_recent_path(&path) {
            return;
        }
        self.recents.retain(|p| p != &path);
        self.recents.insert(0, path);
        self.recents.truncate(10);
        self.recents_cleared = false;
    }
    pub fn merge_persisted_recents(&mut self, persisted: Vec<PathBuf>) {
        let local = self.recents.clone();
        let mut merged = Vec::with_capacity(10);
        let candidates = if self.recents_cleared {
            local
        } else {
            local.into_iter().chain(persisted).collect()
        };
        for path in candidates {
            if valid_recent_path(&path) && !merged.contains(&path) {
                merged.push(path);
                if merged.len() == 10 {
                    break;
                }
            }
        }
        self.recents = merged;
        self.recents_cleared = false;
    }
    pub fn clear_recents(&mut self) {
        self.recents.clear();
        self.recents_cleared = true;
    }
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
    pub fn can_update(&self) -> bool {
        self.active.is_some()
    }
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn name(&self) -> &str {
        self.active.as_ref().map_or("", |s| s.stored.name.as_str())
    }
    pub fn active_favorite_scope(&self) -> task_space::FavoriteScope {
        self.active
            .as_ref()
            .map_or(task_space::FavoriteScope::Global, |space| {
                space.resolved.favorite_scope
            })
    }
}

fn valid_recent_path(path: &std::path::Path) -> bool {
    path.is_absolute()
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("naygospace"))
        && path.as_os_str().len() <= 32_768
}

impl WorkspaceCtrl {
    fn remember_task_space(&mut self, path: PathBuf) {
        self.task_spaces.remember(path);
        self.persist_task_space_recents();
    }

    pub fn spaces_clear_recents(&mut self) {
        self.task_spaces.clear_recents();
        self.persist_task_space_recents();
    }

    pub(super) fn persist_task_space_recents(&mut self) {
        self.config.settings.task_space_recents = self.task_spaces.recents.clone();
        self.config.save();
    }

    pub fn spaces_read_recent(&mut self, index: usize, root: Option<PathBuf>) {
        if !self.task_spaces.open || self.task_spaces.busy() {
            return;
        }
        if let Some(path) = self.task_spaces.recents.get(index).cloned() {
            self.spaces_read(path, root);
        }
    }

    /// Selección rápida desde Disposiciones: abre directamente si el estado actual está limpio;
    /// cuando hay cambios pendientes conserva el flujo explícito guardar/descartar/cancelar.
    pub fn spaces_quick_select_recent(&mut self, index: usize) {
        if !self.spaces_open() {
            return;
        }
        self.task_spaces.quick_apply_if_clean = true;
        self.spaces_read_recent(index, None);
    }
    pub(super) fn space_snapshot(&self, name: &str) -> Result<TaskSpace, SpaceError> {
        self.space_snapshot_with_scope(name, self.task_spaces.active_favorite_scope(), None)
    }

    fn space_snapshot_with_scope(
        &self,
        name: &str,
        favorite_scope: task_space::FavoriteScope,
        favorites: Option<naygo_core::favorites::Favorites>,
    ) -> Result<TaskSpace, SpaceError> {
        let mut space = task_space::from_workspace(name.to_owned(), self.session_persist())?;
        space.workspace.tree_links.sort_unstable();
        space.basket = self.basket.items().to_vec();
        space.saved_searches = self.saved_queries.references.clone();
        space.saved_recipes = self.recipes.references.clone();
        space.typeahead = self.typeahead.clone();
        space.filter_hide_nonmatches = self.filter_hide_nonmatches;
        space.favorite_scope = favorite_scope;
        space.favorites = match favorite_scope {
            task_space::FavoriteScope::Global => None,
            task_space::FavoriteScope::PerSpace => {
                Some(favorites.unwrap_or_else(|| self.favorites.clone()))
            }
        };
        space.visual_filters = self
            .ws
            .panes()
            .iter()
            .filter_map(|p| {
                p.files
                    .as_ref()
                    .and_then(|f| f.visual_filter.clone())
                    .map(|filter| (p.id, filter))
            })
            .collect();
        space.visual_filters.sort_by_key(|(id, _)| *id);
        space.validate()?;
        Ok(space)
    }

    fn space_has_staging(&self) -> bool {
        if self.basket_staging.is_empty() {
            return false;
        }
        self.space_snapshot("snapshot").is_ok_and(|s| {
            self.basket_staging
                .iter()
                .any(|guard| s.references_under(guard.root()))
        })
    }

    // Solo al abrir el gestor o terminar una acción, nunca por fila/tick de render.
    pub(super) fn refresh_space_report(&mut self) {
        self.task_spaces.dirty = self.task_spaces.active.as_ref().is_none_or(|active| {
            self.space_snapshot(&active.resolved.name)
                .and_then(|s| s.signature())
                .ok()
                != active.resolved.signature().ok()
        });
        let current = self.task_spaces.active.as_ref().map_or_else(
            || self.config.t("spaces.unnamed"),
            |s| format!("{}\n{}", s.stored.name, s.path.display()),
        );
        self.task_spaces.report = format!(
            "{}{}\n{}",
            current,
            if self.task_spaces.dirty { " *" } else { "" },
            self.config.t(if self.task_spaces.dirty {
                "spaces.dirty"
            } else {
                "spaces.saved"
            })
        );
        if let Some(pending) = &self.task_spaces.pending {
            self.task_spaces.report.push_str(&format!(
                "\n\n{}: {}\n{}\n{}: {} · {}: {}\n{}",
                self.config.t("spaces.open"),
                pending.stored.name,
                pending.path.display(),
                self.config.t("spaces.panels"),
                pending.resolved.workspace.purposes.len(),
                self.config.t("spaces.references"),
                pending.resolved.basket.len(),
                self.config.t("spaces.confirm")
            ));
        }
    }

    pub fn spaces_open(&mut self) -> bool {
        if self.any_modal_open() || self.pending_drop.is_some() {
            return false;
        }
        self.task_spaces.create_private_favorites = matches!(
            self.task_spaces.active_favorite_scope(),
            task_space::FavoriteScope::PerSpace
        );
        self.task_spaces.open = true;
        self.refresh_space_report();
        true
    }

    pub fn spaces_close(&mut self) {
        if let Some(token) = &self.task_spaces.token {
            token.cancel();
        }
        self.task_spaces.open = false;
        self.task_spaces.pending = None;
        self.task_spaces.accept_read = false;
    }

    fn space_error(&mut self, error: SpaceError) {
        let key = match &error {
            SpaceError::Invalid => "spaces.invalid",
            SpaceError::Version => "spaces.version",
            SpaceError::Limit => "spaces.limit",
            SpaceError::OutsideRoot => "spaces.outside_root",
            SpaceError::Cancelled => "spaces.cancelled",
            SpaceError::Changed => "spaces.changed",
            SpaceError::Exists => "spaces.exists",
            SpaceError::Io(_) => "spaces.io",
        };
        self.refresh_space_report();
        self.task_spaces
            .report
            .push_str(&format!("\n\n{}", self.config.t(key)));
        if let SpaceError::Io(e) = error {
            self.task_spaces.report.push_str(&format!("\n{e}"));
        }
    }

    /// Abre/importa sin aplicar todavía. La revisión requiere confirmación explícita.
    pub fn spaces_read(&mut self, path: PathBuf, root: Option<PathBuf>) {
        if self.task_spaces.busy() {
            return;
        }
        self.task_spaces.pending = None;
        self.task_spaces.accept_read = true;
        let token = CancellationToken::new();
        let worker = token.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = task_space::read(&path, &worker).and_then(|(mut stored, revision)| {
                let resolved = stored.resolved(root.as_deref())?;
                if stored.base_root.is_some() && root.is_some() {
                    stored.base_root = root.clone();
                }
                task_space::checkpoint(&worker)?;
                Ok(Completed::Read(Box::new(SavedSpace {
                    path,
                    revision,
                    stored,
                    resolved,
                })))
            });
            let _ = tx.send(result);
        });
        self.task_spaces.token = Some(token);
        self.task_spaces.rx = Some(rx);
    }

    /// Guardar como/exportar crea un archivo nuevo; actualizar usa la revisión conocida.
    pub fn spaces_save(
        &mut self,
        new_path: Option<PathBuf>,
        root: Option<PathBuf>,
        then_open: bool,
    ) {
        self.spaces_save_with_scope(new_path, root, then_open, None);
    }

    pub fn spaces_save_with_scope(
        &mut self,
        new_path: Option<PathBuf>,
        root: Option<PathBuf>,
        then_open: bool,
        new_favorite_scope: Option<task_space::FavoriteScope>,
    ) {
        let creating_new_space = new_path.is_some();
        if self.task_spaces.busy() {
            return;
        }
        if self.space_has_staging() {
            self.task_spaces.report = self.config.t("spaces.staging");
            return;
        }
        let (path, expected, name, root, favorite_scope) = if let Some(path) = new_path {
            let Some(name) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
                self.space_error(SpaceError::Invalid);
                return;
            };
            (
                path,
                None,
                name,
                root,
                new_favorite_scope.unwrap_or_default(),
            )
        } else if let Some(active) = &self.task_spaces.active {
            (
                active.path.clone(),
                Some(active.revision.clone()),
                active.stored.name.clone(),
                active.stored.base_root.clone(),
                active.resolved.favorite_scope,
            )
        } else {
            return;
        };
        let snapshot = self.space_snapshot_with_scope(
            &name,
            favorite_scope,
            (creating_new_space && matches!(favorite_scope, task_space::FavoriteScope::PerSpace))
                .then(|| self.task_spaces.global_favorites.clone()),
        );
        let (resolved, stored) = match snapshot.and_then(|resolved| {
            let stored = match root {
                Some(root) => resolved.with_relative_paths(&root)?,
                None => resolved.clone(),
            };
            Ok((resolved, stored))
        }) {
            Ok(s) => s,
            Err(e) => {
                self.space_error(e);
                return;
            }
        };
        let token = CancellationToken::new();
        let worker = token.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = naygo_platform::task_space_store::save(
                &path,
                &stored,
                expected.as_deref(),
                &worker,
            )
            .map(|revision| {
                Completed::Written(
                    Box::new(SavedSpace {
                        path,
                        revision,
                        stored,
                        resolved,
                    }),
                    then_open,
                )
            });
            let _ = tx.send(result);
        });
        self.task_spaces.token = Some(token);
        self.task_spaces.rx = Some(rx);
    }

    pub fn pump_task_spaces(&mut self) -> bool {
        let Some(rx) = &self.task_spaces.rx else {
            return true;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return false,
            Err(_) => Err(SpaceError::Invalid),
        };
        self.task_spaces.rx = None;
        self.task_spaces.token = None;
        match result {
            Ok(Completed::Read(space)) => {
                if self.task_spaces.open && self.task_spaces.accept_read {
                    self.task_spaces.pending = Some(*space);
                    if self.task_spaces.quick_apply_if_clean && !self.task_spaces.dirty {
                        self.task_spaces.quick_apply_if_clean = false;
                        self.spaces_apply_pending();
                        return true;
                    }
                }
                self.task_spaces.quick_apply_if_clean = false;
                self.refresh_space_report();
            }
            Ok(Completed::Written(space, then_open)) => {
                self.favorites = match space.resolved.favorite_scope {
                    task_space::FavoriteScope::Global => self.task_spaces.global_favorites.clone(),
                    task_space::FavoriteScope::PerSpace => {
                        space.resolved.favorites.clone().unwrap_or_default()
                    }
                };
                self.remember_task_space(space.path.clone());
                if self
                    .task_spaces
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.path == space.path)
                {
                    self.task_spaces.pending = Some(*space.clone());
                }
                self.task_spaces.active = Some(*space);
                self.refresh_space_report();
                if then_open && self.task_spaces.open {
                    self.spaces_apply_pending();
                }
            }
            Err(error) => self.space_error(error),
        }
        true
    }

    pub fn spaces_apply_pending(&mut self) {
        if self.task_spaces.busy() {
            return;
        }
        let Some(space) = self.task_spaces.pending.take() else {
            return;
        };
        // Validado de nuevo antes de mutar cualquier estado vivo.
        if let Err(error) = space.resolved.validate() {
            self.space_error(error);
            return;
        }
        let Some(workspace) = Workspace::from_persist(&space.resolved.workspace) else {
            return;
        };
        self.favorites = match space.resolved.favorite_scope {
            task_space::FavoriteScope::Global => self.task_spaces.global_favorites.clone(),
            task_space::FavoriteScope::PerSpace => {
                space.resolved.favorites.clone().unwrap_or_default()
            }
        };
        for listing in self.listings.values().chain(self.tree_listings.values()) {
            listing.cancel();
        }
        self.listings.clear();
        self.pending_file_listings.clear();
        self.tree_listings.clear();
        self.pending_tree_listings.clear();
        self.trees.clear();
        self.tree_cursor.clear();
        self.reveal_targets.clear();
        if let Some(job) = self.deep_job.take() {
            job.token.cancel();
        }
        if let Some(job) = self.size_job.take() {
            job.token.cancel();
        }
        self.close_search();
        self.clear_metadata();
        self.preview.set_wanted(None, Instant::now());
        self.autocomplete = Default::default();
        self.search_autocomplete = Default::default();
        self.watchers = crate::watch::Watchers::new();
        self.comparison.clear();
        self.comparison_pair = None;
        self.comparison_link_enabled = false;
        self.comparison_revision = self.comparison_revision.wrapping_add(1);
        self.visit_changes.clear();
        self.visit_changes_revision = self.visit_changes_revision.wrapping_add(1);
        self.pending_recents.clear();
        self.last_listing_dirs.clear();
        self.missing_cache.clear();
        self.missing_probe = None;
        self.ejected_panes.clear();
        self.typeahead.clear();
        self.typeahead_at = None;
        self.filter_match_count = 0;
        self.rename_active = None;
        self.rename_requested = None;
        self.edit_path_requested = None;
        self.last_click = None;
        self.last_open = None;
        self.pending_drop = None;
        self.drag_over_pane = None;
        self.drag_over_row = None;
        self.maximized_pane = None;
        self.basket_import_rx = None;
        self.basket_staging.clear();
        self.basket_missing.clear();
        self.basket_selection = Default::default();
        self.basket_context = false;
        self.search_context = false;
        self.saved_queries = Default::default();
        self.saved_queries.references = space.resolved.saved_searches.clone();
        self.recipes = Default::default();
        self.recipes.references = space.resolved.saved_recipes.clone();
        self.basket.clear();
        self.basket.add(space.resolved.basket.iter().cloned());
        self.ws = workspace;
        self.relaunch_all_panes();
        self.typeahead = space.resolved.typeahead.clone();
        self.filter_hide_nonmatches = space.resolved.filter_hide_nonmatches;
        // start_listing limpia el filtro de tipeo al navegar: restaurarlo DESPUÉS.
        for (id, filter) in &space.resolved.visual_filters {
            if let Some(files) = self.ws.pane_mut(*id).and_then(|p| p.files.as_mut()) {
                files.set_visual_filter(Some(filter.clone()));
            }
        }
        // Cola/historial de operaciones y guards de entregas siguen vivos y globales.
        self.remember_task_space(space.path.clone());
        self.task_spaces.active = Some(space);
        self.task_spaces.open = false;
        self.refresh_space_report();
    }
}
