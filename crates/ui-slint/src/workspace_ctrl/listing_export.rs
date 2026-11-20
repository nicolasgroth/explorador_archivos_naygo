// Naygo — exportación cancelable de instantáneas de listados.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use std::io::{BufWriter, Write};
use std::sync::mpsc::{self, Receiver};

pub(crate) enum ExportDestination {
    Clipboard,
    File(PathBuf),
}

struct ExportSpec {
    entries: Vec<naygo_core::fs_model::Entry>,
    selected: Option<Vec<PathBuf>>,
    visibility: naygo_core::filter::VisibilityFlags,
    columns: Vec<naygo_core::columns::ColumnKind>,
    headers: Vec<String>,
    folder_label: String,
    unavailable_label: String,
    size_format: naygo_core::format::SizeFormat,
    date_format: naygo_core::format::DateFormat,
    tz: i64,
    separator: char,
    destination: ExportDestination,
}

pub(crate) struct ListingExport {
    pane: PaneId,
    root: PathBuf,
    source: ExportSource,
    token: naygo_core::CancellationToken,
    total: usize,
    cursor: usize,
    spec: Option<ExportSpec>,
    receiver: Option<Receiver<Result<Option<String>, String>>>,
    failure: Option<String>,
}

enum ExportSource {
    Deep {
        source_token: naygo_core::CancellationToken,
        view: std::sync::Arc<[usize]>,
        collect_selection: bool,
        selection_revision: u64,
        expected_selection: usize,
    },
    Normal {
        view: std::sync::Arc<[usize]>,
        entries_revision: u64,
        presentation_revision: u64,
        selected: bool,
    },
}

/// Identidad de la vista antes de abrir el diálogo nativo; no copia el listado ni la selección.
pub(crate) struct ExportIntent {
    pane: PaneId,
    root: PathBuf,
    folder_mode: bool,
    menu_identity: std::rc::Rc<()>,
    source: ExportIntentSource,
}

enum ExportIntentSource {
    Deep {
        token: naygo_core::CancellationToken,
        selection_revision: u64,
        view: std::sync::Arc<[usize]>,
    },
    Normal {
        view: std::sync::Arc<[usize]>,
        entries_revision: u64,
        presentation_revision: u64,
    },
}

impl WorkspaceCtrl {
    pub(crate) fn capture_export_intent(&self) -> Option<ExportIntent> {
        let menu = self.context_menu.as_ref()?;
        let pane = self.ws.pane(menu.pane)?.files.as_ref()?;
        let root = pane.current_dir.clone();
        if menu.folder_mode && menu.targets.first() != Some(&root) {
            return None;
        }
        let source = if self.is_deep_active(menu.pane) {
            let job = self.deep_job.as_ref().filter(|job| {
                job.pane == menu.pane
                    && job.root == root
                    && job.done
                    && !job.partial
                    && !job.cancelled
            })?;
            ExportIntentSource::Deep {
                token: job.token.clone(),
                selection_revision: job.selection_revision,
                view: self.deep_view_indices(menu.pane)?,
            }
        } else {
            ExportIntentSource::Normal {
                view: pane.cached_view_indices_shared()?,
                entries_revision: pane.entries_revision(),
                presentation_revision: pane.presentation_revision(),
            }
        };
        Some(ExportIntent {
            pane: menu.pane,
            root,
            folder_mode: menu.folder_mode,
            menu_identity: menu.identity.clone(),
            source,
        })
    }

    pub(crate) fn export_intent_is_current(&self, intent: &ExportIntent) -> bool {
        let Some(menu) = self.context_menu.as_ref().filter(|menu| {
            menu.pane == intent.pane
                && menu.folder_mode == intent.folder_mode
                && std::rc::Rc::ptr_eq(&menu.identity, &intent.menu_identity)
        }) else {
            return false;
        };
        let Some(pane) = self
            .ws
            .pane(intent.pane)
            .and_then(|pane| pane.files.as_ref())
            .filter(|pane| pane.current_dir == intent.root)
        else {
            return false;
        };
        if menu.folder_mode && menu.targets.first() != Some(&intent.root) {
            return false;
        }
        match &intent.source {
            ExportIntentSource::Deep {
                token,
                selection_revision,
                view,
            } => self.deep_job.as_ref().is_some_and(|job| {
                job.pane == intent.pane
                    && job.root == intent.root
                    && job.done
                    && !job.cancelled
                    && !job.partial
                    && !token.is_cancelled()
                    && job.selection_revision == *selection_revision
                    && self
                        .cached_deep_view_indices(intent.pane)
                        .is_some_and(|current| std::sync::Arc::ptr_eq(&current, view))
            }),
            ExportIntentSource::Normal {
                view,
                entries_revision,
                presentation_revision,
            } => {
                !self.is_deep_active(intent.pane)
                    && pane.entries_revision() == *entries_revision
                    && pane.presentation_revision() == *presentation_revision
                    && pane
                        .cached_view_indices_shared()
                        .is_some_and(|current| std::sync::Arc::ptr_eq(&current, view))
            }
        }
    }

    pub(crate) fn reject_listing_export(&mut self) {
        self.close_context_menu();
        self.pending_listing_error = Some(self.config.t("export.unavailable"));
    }

    /// Captura el menú propietario y sus rutas; la instantánea se copia en fragmentos en ticks.
    pub(crate) fn start_listing_export(
        &mut self,
        separator: char,
        destination: ExportDestination,
    ) -> bool {
        if self.listing_export.is_some() {
            return false;
        }
        let Some(menu) = self.context_menu.take() else {
            return false;
        };
        let Some(pane) = self.ws.pane(menu.pane).and_then(|pane| pane.files.as_ref()) else {
            return false;
        };
        let root = pane.current_dir.clone();
        if menu.folder_mode && menu.targets.first() != Some(&root) {
            self.pending_listing_error = Some(self.config.t("export.unavailable"));
            return false;
        }
        let columns: Vec<_> = pane
            .table
            .visible_columns()
            .map(|column| column.kind)
            .collect();
        let mut headers: Vec<_> = columns
            .iter()
            .map(|kind| {
                use naygo_core::columns::ColumnKind::*;
                self.config.t(match kind {
                    Name => "col.name",
                    Extension => "col.extension",
                    Size => "col.size",
                    Modified => "col.modified",
                    Created => "col.created",
                })
            })
            .collect();
        headers.push(self.config.t("export.path"));
        let (source, total, selected, visibility) = if self.is_deep_active(menu.pane) {
            let Some(job) = self.deep_job.as_ref().filter(|job| {
                job.pane == menu.pane
                    && job.root == root
                    && job.done
                    && !job.cancelled
                    && !job.partial
            }) else {
                self.pending_listing_error = Some(self.config.t("export.unavailable"));
                return false;
            };
            let collect_selection = menu.folder_mode && !job.selected_paths.is_empty();
            let Some(view) = self.deep_view_indices(menu.pane) else {
                self.pending_listing_error = Some(self.config.t("export.unavailable"));
                return false;
            };
            let total = view.len();
            let expected_selection = job.selected_paths.len();
            let selected = if collect_selection {
                Some(Vec::with_capacity(expected_selection))
            } else if menu.folder_mode {
                None
            } else {
                Some(menu.targets)
            };
            (
                ExportSource::Deep {
                    source_token: job.token.clone(),
                    view,
                    collect_selection,
                    selection_revision: job.selection_revision,
                    expected_selection,
                },
                total,
                selected,
                self.visibility_flags(),
            )
        } else {
            // Nunca recalcular la vista (que puede ordenar cientos de miles de filas) en el
            // callback. Si aún no está materializada, rechazar y permitir reintentar.
            let Some(view) = pane.cached_view_indices_shared() else {
                self.pending_listing_error = Some(self.config.t("export.unavailable"));
                return false;
            };
            let selected = !pane.selected.is_empty();
            let total = if selected {
                pane.selected.len()
            } else {
                view.len()
            };
            let expected = if selected && !menu.folder_mode {
                if menu.targets.len() != total {
                    self.pending_listing_error = Some(self.config.t("export.unavailable"));
                    return false;
                }
                Some(menu.targets)
            } else {
                None
            };
            (
                ExportSource::Normal {
                    view,
                    entries_revision: pane.entries_revision(),
                    presentation_revision: pane.presentation_revision(),
                    selected,
                },
                total,
                expected,
                naygo_core::filter::VisibilityFlags::default(),
            )
        };
        let spec = ExportSpec {
            entries: Vec::with_capacity(total),
            selected,
            visibility,
            columns,
            headers,
            folder_label: self.config.t("icons.obj.folder"),
            unavailable_label: self.config.t("export.unavailable"),
            size_format: self.config.settings.size_format,
            date_format: self.config.settings.date_format,
            tz: self.tz_offset_secs(),
            separator,
            destination,
        };
        self.listing_export = Some(ListingExport {
            pane: menu.pane,
            root,
            source,
            token: naygo_core::CancellationToken::new(),
            total,
            cursor: 0,
            spec: Some(spec),
            receiver: None,
            failure: None,
        });
        true
    }

    pub(crate) fn cancel_listing_export(&mut self) {
        if let Some(export) = self.listing_export.as_ref() {
            export.token.cancel();
        }
    }

    pub(crate) fn pump_listing_export(&mut self) -> bool {
        let Some(mut export) = self.listing_export.take() else {
            return true;
        };
        if let Some(receiver) = export.receiver.as_ref() {
            match receiver.try_recv() {
                Ok(Ok(Some(csv))) if !export.token.is_cancelled() => {
                    self.ops.write_clipboard_owned(csv);
                }
                // El commit puede ganar la carrera contra Cancelar: informar que el archivo
                // ya se guardó, nunca presentar esa operación como cancelada.
                Ok(Ok(None)) => {
                    self.pending_listing_error = Some(self.config.t("export.saved"));
                }
                Ok(Err(error)) if !export.token.is_cancelled() => {
                    self.pending_listing_error =
                        Some(format!("{}: {error}", self.config.t("ops.plan_failed")));
                }
                Ok(_) => {
                    self.pending_listing_error = Some(self.config.t("export.cancelled"));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    if !export.token.is_cancelled() {
                        self.pending_listing_error = Some(self.config.t("ops.plan_failed"));
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.listing_export = Some(export);
                    return false;
                }
            }
            return true;
        }
        if export.token.is_cancelled() {
            // Evitar liberar decenas de miles de rutas de golpe en el hilo de UI.
            if let Some(spec) = export.spec.as_mut() {
                spec.entries
                    .truncate(spec.entries.len().saturating_sub(256));
                if let Some(paths) = spec.selected.as_mut() {
                    paths.truncate(paths.len().saturating_sub(256));
                }
                if !spec.entries.is_empty()
                    || spec
                        .selected
                        .as_ref()
                        .is_some_and(|paths| !paths.is_empty())
                {
                    self.listing_export = Some(export);
                    return false;
                }
            }
            if let Some(error) = export.failure.take() {
                self.pending_listing_error = Some(error);
            } else {
                self.pending_listing_error = Some(self.config.t("export.cancelled"));
            }
            return true;
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2);
        let end = export.total.min(export.cursor.saturating_add(256));
        let mut invalid = false;
        match &export.source {
            ExportSource::Deep {
                source_token,
                view,
                collect_selection,
                selection_revision,
                ..
            } => {
                let job = self.deep_job.as_ref().filter(|job| {
                    job.pane == export.pane
                        && job.root == export.root
                        && job.done
                        && !job.partial
                        && !job.cancelled
                        && !source_token.is_cancelled()
                        && self
                            .cached_deep_view_indices(export.pane)
                            .is_some_and(|current| std::sync::Arc::ptr_eq(&current, view))
                        && (!*collect_selection || job.selection_revision == *selection_revision)
                });
                if let Some(job) = job {
                    while export.cursor < end && std::time::Instant::now() < deadline {
                        let entry = &job.items[view[export.cursor]].0;
                        let spec = export.spec.as_mut().expect("snapshot activo");
                        if *collect_selection && job.selected_paths.contains(&entry.path) {
                            spec.selected
                                .as_mut()
                                .expect("selección activa")
                                .push(entry.path.clone());
                        }
                        spec.entries.push(entry.clone());
                        export.cursor += 1;
                    }
                } else {
                    invalid = true;
                }
            }
            ExportSource::Normal {
                view,
                entries_revision,
                presentation_revision,
                selected,
            } => {
                let pane = self
                    .ws
                    .pane(export.pane)
                    .and_then(|pane| pane.files.as_ref())
                    .filter(|pane| {
                        pane.current_dir == export.root
                            && !self.is_deep_active(export.pane)
                            && pane.entries_revision() == *entries_revision
                            && (!*selected
                                || pane.presentation_revision() == *presentation_revision)
                            && pane
                                .cached_view_indices_shared()
                                .is_some_and(|current| std::sync::Arc::ptr_eq(&current, view))
                    });
                if let Some(pane) = pane {
                    while export.cursor < end && std::time::Instant::now() < deadline {
                        let pos = if *selected {
                            pane.selected.get(export.cursor).copied()
                        } else {
                            Some(export.cursor)
                        };
                        let entry = pos
                            .and_then(|pos| view.get(pos))
                            .and_then(|real| pane.entries.get(*real));
                        let Some(entry) = entry else {
                            invalid = true;
                            break;
                        };
                        let spec = export.spec.as_mut().expect("snapshot activo");
                        if spec
                            .selected
                            .as_ref()
                            .is_some_and(|paths| paths.get(export.cursor) != Some(&entry.path))
                        {
                            invalid = true;
                            break;
                        }
                        spec.entries.push(entry.clone());
                        export.cursor += 1;
                    }
                } else {
                    invalid = true;
                }
            }
        }
        if invalid {
            export.failure = Some(self.config.t("export.unavailable"));
            export.token.cancel();
            self.listing_export = Some(export);
            return false;
        }
        if export.cursor == export.total {
            let spec = export.spec.take().expect("snapshot activo");
            let incomplete = matches!(&export.source, ExportSource::Deep { collect_selection: true, expected_selection, .. }
                if spec.selected.as_ref().map_or(0, Vec::len) != *expected_selection);
            if incomplete {
                export.spec = Some(spec);
                export.failure = Some(self.config.t("export.unavailable"));
                export.token.cancel();
                self.listing_export = Some(export);
                return false;
            }
            let token = export.token.clone();
            let (tx, rx) = mpsc::channel();
            match std::thread::Builder::new()
                .name("deep-export".into())
                .spawn(move || {
                    let _ = tx.send(export_worker(spec, &token));
                }) {
                Ok(_) => export.receiver = Some(rx),
                Err(error) => {
                    self.pending_listing_error =
                        Some(format!("{}: {error}", self.config.t("ops.plan_failed")));
                    return true;
                }
            }
        }
        self.listing_export = Some(export);
        false
    }
}

fn export_worker(
    spec: ExportSpec,
    token: &naygo_core::CancellationToken,
) -> Result<Option<String>, String> {
    let selected = if let Some(paths) = spec.selected.as_ref() {
        let mut selected = std::collections::HashSet::with_capacity(paths.len());
        for path in paths {
            if token.is_cancelled() {
                return Err("cancelled".into());
            }
            selected.insert(path.clone());
        }
        Some(selected)
    } else {
        None
    };
    let write_rows = |writer: &mut dyn Write| -> Result<(), String> {
        writer
            .write_all(
                naygo_core::listing_export::to_csv(&spec.headers, &[], spec.separator).as_bytes(),
            )
            .map_err(|e| e.to_string())?;
        let mut matched = 0usize;
        for entry in &spec.entries {
            if token.is_cancelled() {
                return Err("cancelled".into());
            }
            if !spec.visibility.allows(entry)
                || selected
                    .as_ref()
                    .is_some_and(|paths| !paths.contains(&entry.path))
            {
                continue;
            }
            matched += 1;
            let mut row: Vec<_> = spec
                .columns
                .iter()
                .map(|kind| {
                    if *kind == naygo_core::columns::ColumnKind::Extension && entry.is_dir() {
                        spec.folder_label.clone()
                    } else {
                        crate::bridge::cell_value(
                            entry,
                            *kind,
                            spec.size_format,
                            spec.date_format,
                            spec.tz,
                        )
                    }
                })
                .collect();
            row.push(entry.path.display().to_string());
            writer
                .write_all(naygo_core::listing_export::to_csv(&row, &[], spec.separator).as_bytes())
                .map_err(|e| e.to_string())?;
        }
        if selected
            .as_ref()
            .is_some_and(|paths| matched != paths.len())
        {
            return Err(spec.unavailable_label.clone());
        }
        if token.is_cancelled() {
            return Err("cancelled".into());
        }
        Ok(())
    };
    match &spec.destination {
        ExportDestination::Clipboard => {
            let mut bytes = Vec::new();
            write_rows(&mut bytes)?;
            Ok(Some(String::from_utf8(bytes).map_err(|e| e.to_string())?))
        }
        ExportDestination::File(path) => {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| std::path::Path::new("."));
            let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
            {
                let mut writer = BufWriter::new(temp.as_file_mut());
                writer
                    .write_all(&[0xef, 0xbb, 0xbf])
                    .map_err(|e| e.to_string())?;
                write_rows(&mut writer)?;
                writer.flush().map_err(|e| e.to_string())?;
            }
            if token.is_cancelled() {
                return Err("cancelled".into());
            }
            temp.persist(path).map_err(|e| e.error.to_string())?;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(destination: PathBuf) -> ExportSpec {
        ExportSpec {
            entries: vec![],
            selected: Some(vec![PathBuf::from("ausente")]),
            visibility: naygo_core::filter::VisibilityFlags::default(),
            columns: vec![],
            headers: vec!["Ruta".into()],
            folder_label: "Carpeta".into(),
            unavailable_label: "incompleto".into(),
            size_format: Default::default(),
            date_format: Default::default(),
            tz: 0,
            separator: ';',
            destination: ExportDestination::File(destination),
        }
    }

    #[test]
    fn exportacion_incompleta_o_cancelada_conserva_destino_y_limpia_temporal() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("listado.csv");
        std::fs::write(&target, b"original").unwrap();
        let token = naygo_core::CancellationToken::new();
        assert_eq!(
            export_worker(spec(target.clone()), &token).unwrap_err(),
            "incompleto"
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"original");
        token.cancel();
        assert!(export_worker(spec(target.clone()), &token).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"original");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn snapshot_largo_se_copia_por_fragmentos_y_se_puede_cancelar() {
        let root = tempfile::tempdir().unwrap();
        let cfg = tempfile::tempdir().unwrap();
        let mut ctrl = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
        let pane = ctrl.active_id().unwrap();
        ctrl.deep_start(pane);
        for _ in 0..4000 {
            ctrl.deep_poll();
            if ctrl.deep_job.as_ref().is_some_and(|job| job.done) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let job = ctrl.deep_job.as_mut().unwrap();
        assert!(job.done && !job.partial);
        *job.items = (0..700)
            .map(|i| {
                let path = root.path().join(format!("{i}.txt"));
                (
                    naygo_core::fs_model::Entry {
                        name: format!("{i}.txt"),
                        path,
                        kind: naygo_core::fs_model::EntryKind::File,
                        size: Some(1),
                        modified: None,
                        created: None,
                        hidden: false,
                        system: false,
                    },
                    0,
                )
            })
            .collect();
        ctrl.open_folder_context_menu(pane, 0.0, 0.0);
        assert!(ctrl.start_listing_export(';', ExportDestination::Clipboard));
        assert!(!ctrl.pump_listing_export());
        assert!(ctrl.listing_export.as_ref().unwrap().cursor <= 256);
        ctrl.cancel_listing_export();
        for _ in 0..20 {
            if ctrl.pump_listing_export() {
                break;
            }
        }
        assert!(ctrl.listing_export.is_none());
    }

    #[test]
    fn snapshot_normal_largo_es_incremental_y_rechaza_reordenamiento() {
        let root = tempfile::tempdir().unwrap();
        let cfg = tempfile::tempdir().unwrap();
        let mut ctrl = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
        let pane = ctrl.active_id().unwrap();
        let files = ctrl.ws.active_files_mut().unwrap();
        files.entries = (0..700)
            .map(|i| {
                let path = root.path().join(format!("{i}.txt"));
                naygo_core::fs_model::Entry {
                    name: format!("{i}.txt"),
                    path,
                    kind: naygo_core::fs_model::EntryKind::File,
                    size: Some(1),
                    modified: None,
                    created: None,
                    hidden: false,
                    system: false,
                }
            })
            .collect();
        files.entries_changed();
        files.view_indices_shared();
        ctrl.open_folder_context_menu(pane, 0.0, 0.0);
        assert!(ctrl.start_listing_export(';', ExportDestination::Clipboard));
        assert!(!ctrl.pump_listing_export());
        assert!(ctrl.listing_export.as_ref().unwrap().cursor <= 256);
        ctrl.ws.active_files_mut().unwrap().entries_changed();
        for _ in 0..20 {
            if ctrl.pump_listing_export() {
                break;
            }
        }
        assert!(ctrl.listing_export.is_none());
        assert!(ctrl.pending_listing_error.is_some());
    }

    #[test]
    fn cancelacion_tardia_no_oculta_que_archivo_ya_fue_guardado() {
        let root = tempfile::tempdir().unwrap();
        let cfg = tempfile::tempdir().unwrap();
        let mut ctrl = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(None)).unwrap();
        let token = naygo_core::CancellationToken::new();
        token.cancel();
        ctrl.listing_export = Some(ListingExport {
            pane: ctrl.active_id().unwrap(),
            root: root.path().to_path_buf(),
            source: ExportSource::Deep {
                source_token: naygo_core::CancellationToken::new(),
                view: Default::default(),
                collect_selection: false,
                selection_revision: 0,
                expected_selection: 0,
            },
            token,
            total: 0,
            cursor: 0,
            spec: None,
            receiver: Some(rx),
            failure: None,
        });
        assert!(ctrl.pump_listing_export());
        assert_eq!(
            ctrl.pending_listing_error.as_deref(),
            Some(ctrl.config.t("export.saved").as_str())
        );
    }
}
