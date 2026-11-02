// Naygo — carga de configuración y workspace al iniciar sin bloquear la UI.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Lee el estado persistido en un worker. El resultado es un snapshot propio que la UI aplica
//! más tarde en su event loop; nunca se captura Slint ni un controlador en el worker.

use naygo_core::{
    config::{self, Settings, WorkspacePersist},
    favorites::Favorites,
    i18n::I18n,
    keymap::KeyMap,
    workspace::TemplateStore,
};
use std::{path::PathBuf, sync::mpsc, thread};

pub(crate) struct StartupData {
    pub settings: Settings,
    pub i18n: I18n,
    pub keymap: KeyMap,
    pub workspace: Option<WorkspacePersist>,
    pub favorites: Favorites,
    pub templates: TemplateStore,
    pub first_launch_save_ticket: Option<config::SaveTicket>,
}

pub(crate) fn spawn(
    config_dir: PathBuf,
    waker: naygo_platform::dir_watch::Waker,
) -> mpsc::Receiver<Result<StartupData, String>> {
    let (tx, rx) = mpsc::sync_channel(1);
    let spawn_error_tx = tx.clone();
    let spawn_error_waker = waker.clone();
    let spawn_result = thread::Builder::new()
        .name("naygo-startup-config".into())
        .spawn(move || {
            let result = load(&config_dir);
            match tx.send(result) {
                Ok(()) => waker(),
                Err(error) => crate::logging::log_line(&format!(
                    "se descartó resultado de carga inicial: {error}"
                )),
            }
        });
    if let Err(error) = spawn_result {
        let _ = spawn_error_tx.send(Err(format!(
            "no se pudo iniciar worker de configuración: {error}"
        )));
        spawn_error_waker();
    }
    rx
}

fn load(config_dir: &std::path::Path) -> Result<StartupData, String> {
    let first_launch = !config_dir.join("settings.json").exists();
    let (mut settings, settings_recovered) =
        config::load_settings_flagged_deferred_icon_set(config_dir);
    let mut i18n = I18n::load(config_dir, &settings.language);
    let mut first_launch_save_ticket = None;
    if first_launch {
        if let Some(locale) = naygo_platform::locale::os_locale() {
            let language = naygo_core::i18n::pick_default_language(&locale, i18n.available());
            if i18n.set_language(&language) {
                settings.language = language;
                first_launch_save_ticket = match config::save_settings_async(config_dir, &settings)
                {
                    Ok(ticket) => Some(ticket),
                    Err(error) => {
                        crate::logging::log_line(&format!(
                            "no se pudo encolar la persistencia del idioma inicial: {error}"
                        ));
                        None
                    }
                };
            }
        }
    }

    let (workspace, workspace_recovered) = config::load_workspace_flagged(config_dir);
    let favorites = load_favorites(config_dir);
    let templates = config::load_templates(config_dir);
    let keymap = config::load_keymap(config_dir);

    if settings_recovered {
        crate::logging::log_line("settings.json estaba corrupto; se respaldó y se usaron defaults");
    }
    if workspace_recovered {
        crate::logging::log_line(
            "workspace.json estaba corrupto; se respaldó y se usará el layout inicial",
        );
    }

    Ok(StartupData {
        settings,
        i18n,
        keymap,
        workspace,
        favorites,
        templates,
        first_launch_save_ticket,
    })
}

fn load_favorites(config_dir: &std::path::Path) -> Favorites {
    let path = naygo_core::favorites::favorites_path(config_dir);
    match std::fs::read_to_string(path) {
        Ok(json) => Favorites::from_json(&json),
        Err(_) => Favorites::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn worker_carga_defaults_sin_archivos_y_devuelve_snapshot_completo() {
        let dir = tempdir().unwrap();
        let data = load(dir.path()).expect("el arranque tolera un directorio vacío");
        assert_eq!(data.settings.version, Settings::default().version);
        assert!(data.workspace.is_none());
        assert_eq!(data.favorites, Favorites::default());
        assert_eq!(data.templates, TemplateStore::default());
        assert_eq!(data.keymap, KeyMap::defaults());
        assert!(!data.i18n.available().is_empty());
    }

    #[test]
    fn worker_lee_todos_los_archivos_persistidos() {
        let dir = tempdir().unwrap();
        let settings = Settings::default();
        config::save_settings(dir.path(), &settings);
        let keymap = KeyMap::defaults();
        config::save_keymap(dir.path(), &keymap);
        let favorites = Favorites::new();
        std::fs::write(
            naygo_core::favorites::favorites_path(dir.path()),
            favorites.to_json(),
        )
        .unwrap();
        config::save_templates(dir.path(), &TemplateStore::default());
        let workspace = WorkspacePersist {
            version: config::CONFIG_VERSION,
            layout: naygo_core::workspace::layout::SerializableDockLayout::single(
                naygo_core::workspace::PaneId(1),
            ),
            active: None,
            files: Vec::new(),
            purposes: Vec::new(),
            tree_links: Vec::new(),
            operations_placement: None,
        };
        config::save_workspace(dir.path(), &workspace);

        let data = load(dir.path()).expect("carga completa");
        assert_eq!(data.settings, settings);
        assert_eq!(data.keymap, keymap);
        assert_eq!(
            data.workspace.map(|value| value.version),
            Some(workspace.version)
        );
    }
}
