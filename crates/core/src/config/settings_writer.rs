// Naygo — escritura JSON asíncrona, ordenada y coalescida para configuración.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    time::Duration,
};

#[derive(Default)]
struct Slot {
    revision: AtomicU64,
    writing: Mutex<()>,
}

struct PendingJson {
    path: PathBuf,
    contents: String,
    slot: Arc<Slot>,
    revision: u64,
}

#[derive(Default)]
struct QueueState {
    // Una sola escritura pendiente por archivo: una edición nueva reemplaza la anterior.
    pending: HashMap<PathBuf, PendingJson>,
    finished: HashMap<PathBuf, u64>,
    errors: HashMap<PathBuf, (u64, String)>,
}

struct Queue {
    state: Mutex<QueueState>,
    ready: Condvar,
    slots: Mutex<HashMap<PathBuf, Arc<Slot>>>,
}

static QUEUE: OnceLock<Arc<Queue>> = OnceLock::new();

/// Identifica una versión concreta solicitada al escritor asíncrono.
#[derive(Clone, Debug)]
pub struct SaveTicket {
    path: PathBuf,
    revision: u64,
}

impl SaveTicket {
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

fn queue() -> &'static Arc<Queue> {
    QUEUE.get_or_init(|| {
        let queue = Arc::new(Queue {
            state: Mutex::new(QueueState::default()),
            ready: Condvar::new(),
            slots: Mutex::new(HashMap::new()),
        });
        let worker = Arc::clone(&queue);
        let _ = std::thread::Builder::new()
            .name("naygo-config-writer".into())
            .spawn(move || writer_loop(worker));
        queue
    })
}

fn writer_loop(queue: Arc<Queue>) {
    loop {
        let pending = {
            let mut state = queue.state.lock().unwrap_or_else(|e| e.into_inner());
            while state.pending.is_empty() {
                state = queue.ready.wait(state).unwrap_or_else(|e| e.into_inner());
            }
            let path = state
                .pending
                .keys()
                .next()
                .cloned()
                .expect("pending is not empty");
            state.pending.remove(&path).expect("pending path exists")
        };

        let path = pending.path.clone();
        let revision = pending.revision;
        let result = pending.write();
        let mut state = queue.state.lock().unwrap_or_else(|e| e.into_inner());
        state
            .finished
            .entry(path.clone())
            .and_modify(|v| *v = (*v).max(revision))
            .or_insert(revision);
        match result {
            Ok(()) => {
                if state
                    .errors
                    .get(&path)
                    .is_some_and(|(failed, _)| *failed <= revision)
                {
                    state.errors.remove(&path);
                }
            }
            Err(error) => {
                tracing::warn!("falló escritura JSON asíncrona: {error}");
                state.errors.insert(path, (revision, error));
            }
        }
        queue.ready.notify_all();
    }
}

impl PendingJson {
    fn prepare<T: Serialize>(path: PathBuf, value: &T) -> Result<Self, String> {
        let contents = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
        let queue = queue();
        let mut slots = queue.slots.lock().unwrap_or_else(|e| e.into_inner());
        let slot = Arc::clone(slots.entry(path.clone()).or_default());
        let revision = slot.revision.fetch_add(1, Ordering::AcqRel) + 1;
        Ok(Self {
            path,
            contents,
            slot,
            revision,
        })
    }

    fn enqueue(self) -> SaveTicket {
        let ticket = SaveTicket {
            path: self.path.clone(),
            revision: self.revision,
        };
        let queue = queue();
        let mut state = queue.state.lock().unwrap_or_else(|e| e.into_inner());
        state.pending.insert(self.path.clone(), self);
        queue.ready.notify_one();
        ticket
    }

    fn write(self) -> Result<(), String> {
        let _lock = self.slot.writing.lock().unwrap_or_else(|e| e.into_inner());
        if self.slot.revision.load(Ordering::Acquire) != self.revision {
            return Ok(());
        }
        crate::fs_util::write_atomic(&self.path, &self.contents)
            .map_err(|e| format!("{}: {e}", self.path.display()))
    }
}

/// Serializa en memoria y entrega el guardado al único worker. Si llegan cambios rápidos,
/// conserva sólo la última revisión pendiente para cada archivo.
pub(super) fn save_async<T: Serialize>(path: &Path, value: &T) -> Result<SaveTicket, String> {
    Ok(PendingJson::prepare(path.to_path_buf(), value)?.enqueue())
}

/// Espera una revisión fuera del hilo de UI. Una revisión posterior satisface el ticket anterior.
pub(super) fn wait(ticket: &SaveTicket, timeout: Duration) -> Result<(), String> {
    let queue = queue();
    let deadline = std::time::Instant::now() + timeout;
    let mut state = queue.state.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if state
            .finished
            .get(&ticket.path)
            .is_some_and(|v| *v >= ticket.revision)
        {
            if let Some((revision, error)) = state.errors.get(&ticket.path) {
                if *revision >= ticket.revision {
                    return Err(error.clone());
                }
            }
            return Ok(());
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return Err("timeout esperando el guardado de configuración".to_string());
        }
        let (next, result) = queue
            .ready
            .wait_timeout(state, deadline.saturating_duration_since(now))
            .unwrap_or_else(|e| e.into_inner());
        state = next;
        if result.timed_out()
            && state
                .finished
                .get(&ticket.path)
                .is_none_or(|v| *v < ticket.revision)
        {
            return Err("timeout esperando el guardado de configuración".to_string());
        }
    }
}

/// Serializa en memoria y escribe de inmediato. Se conserva para inicialización y llamadas fuera
/// de callbacks de UI; los consumidores interactivos deben usar `save_async`.
pub(super) fn save_sync<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    PendingJson::prepare(path.to_path_buf(), value)?.write()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{load_settings_flagged, save_settings};

    #[test]
    fn delayed_old_settings_cannot_overwrite_newer_sync_save() {
        let dir = tempfile::tempdir().unwrap();
        let mut old = super::super::Settings::default();
        old.fonts.set(0, "Tahoma");
        let stale = PendingJson::prepare(dir.path().join("settings.json"), &old).unwrap();
        let mut latest = old.clone();
        latest.icon_only = false;
        save_settings(dir.path(), &latest);
        stale.write().unwrap();
        let loaded = load_settings_flagged(dir.path()).0;
        assert!(!loaded.icon_only);
        assert_eq!(loaded.fonts.interface, "Tahoma");
    }

    #[test]
    fn async_settings_save_can_be_waited_and_persists_latest_revision() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = super::super::Settings::default();
        first.fonts.set(1, "Verdana");
        let stale = save_async(&dir.path().join("settings.json"), &first).unwrap();
        let mut latest = first;
        latest.icon_only = false;
        let current = save_async(&dir.path().join("settings.json"), &latest).unwrap();
        assert!(current.revision() > stale.revision());
        wait(&current, Duration::from_secs(5)).unwrap();
        let third = save_async(&dir.path().join("settings.json"), &latest).unwrap();
        assert!(third.revision() > current.revision());
        wait(&third, Duration::from_secs(5)).unwrap();
        let loaded = load_settings_flagged(dir.path()).0;
        assert!(!loaded.icon_only);
        assert_eq!(loaded.fonts.listings, "Verdana");
    }

    #[test]
    fn async_settings_save_reports_write_errors() {
        let dir = tempfile::tempdir().unwrap();
        let settings = super::super::Settings::default();
        let blocker = dir.path().join("file-blocking-directory");
        std::fs::write(&blocker, b"not a directory").unwrap();
        let ticket = save_async(&blocker.join("settings.json"), &settings).unwrap();
        let error = wait(&ticket, Duration::from_secs(5)).unwrap_err();
        assert!(error.contains("file-blocking-directory"));
    }
}
