// Naygo — guardado diferido de ajustes sin sobrescribir una revisión más reciente.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::Settings;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock, Weak,
    },
};

#[derive(Default)]
struct Slot {
    revision: AtomicU64,
    writing: Mutex<()>,
}

static SLOTS: OnceLock<Mutex<HashMap<PathBuf, Weak<Slot>>>> = OnceLock::new();

pub(super) struct PendingSettings {
    path: PathBuf,
    settings: Settings,
    slot: Arc<Slot>,
    revision: u64,
}

impl PendingSettings {
    /// Sólo memoria: reservar orden ANTES de lanzar el worker, no al empezar el I/O.
    pub(super) fn prepare(dir: &Path, settings: &Settings) -> Self {
        let path = dir.join("settings.json");
        let mut slots = SLOTS
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        slots.retain(|_, slot| slot.strong_count() > 0);
        let slot = slots.get(&path).and_then(Weak::upgrade).unwrap_or_else(|| {
            let slot = Arc::new(Slot::default());
            slots.insert(path.clone(), Arc::downgrade(&slot));
            slot
        });
        let revision = slot.revision.fetch_add(1, Ordering::AcqRel) + 1;
        Self {
            path,
            settings: settings.clone(),
            slot,
            revision,
        }
    }

    pub(super) fn write(self) {
        let _lock = self.slot.writing.lock().unwrap_or_else(|e| e.into_inner());
        if self.slot.revision.load(Ordering::Acquire) == self.revision {
            super::write_json(&self.path, &self.settings);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_font_save_cannot_overwrite_newer_settings() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        settings.fonts.set(0, "Tahoma");
        let delayed = PendingSettings::prepare(dir.path(), &settings);
        settings.icon_only = false;
        super::super::save_settings(dir.path(), &settings);
        delayed.write();
        let loaded = super::super::load_settings_flagged(dir.path()).0;
        assert!(!loaded.icon_only);
        assert_eq!(loaded.fonts.interface, "Tahoma");
    }

    #[test]
    fn background_settings_save_persists_fonts() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        settings.fonts.set(1, "Verdana");
        super::super::save_settings_async(dir.path(), &settings)
            .join()
            .unwrap();
        let loaded = super::super::load_settings_flagged(dir.path()).0;
        assert_eq!(loaded.fonts.listings, "Verdana");
    }
}
