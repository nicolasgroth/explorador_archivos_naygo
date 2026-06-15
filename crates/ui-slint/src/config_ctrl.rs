// Naygo — controlador de configuración de la UI Slint (Fase 4). Posee los Settings, el
// catálogo i18n, el catálogo de temas y el mapa de atajos, todo cargado desde el core, y los
// persiste en el directorio portable. La UI (ventana de config, editor de atajos) habla con
// este controlador; nunca toca el disco directo.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// API foundacional de la Fase 4: este módulo se introduce completo, pero sus consumidores
// (theme_apply, i18n_keys, la ventana de configuración y el editor de atajos) se cablean en
// las tareas siguientes de la misma fase. Hasta entonces algunos métodos quedan sin llamador
// desde el binario, de ahí el allow a nivel de módulo. Se quita al cerrar la fase.
#![allow(dead_code)]

use naygo_core::config::{self, Settings};
use naygo_core::i18n::{I18n, LangId};
use naygo_core::keymap::KeyMap;
use naygo_core::theme::{Theme, ThemeCatalog, ThemeId};
use std::path::PathBuf;

/// Estado de configuración de la app: ajustes + idioma + temas + atajos.
pub struct ConfigCtrl {
    pub settings: Settings,
    pub i18n: I18n,
    pub themes: ThemeCatalog,
    pub keymap: KeyMap,
    pub config_dir: PathBuf,
}

impl ConfigCtrl {
    /// Carga todo desde `config_dir` (settings.json, lang/, themes/, keymap.json). Un
    /// directorio vacío o archivos corruptos caen a defaults sin panic.
    pub fn new(config_dir: PathBuf) -> ConfigCtrl {
        let (settings, _recovered) = config::load_settings_flagged(&config_dir);
        let i18n = I18n::load(&config_dir, &settings.language);
        let themes = ThemeCatalog::load(&config_dir, &settings.theme);
        let keymap = config::load_keymap(&config_dir);
        ConfigCtrl {
            settings,
            i18n,
            themes,
            keymap,
            config_dir,
        }
    }

    /// Texto traducido al idioma activo (atajo a `i18n.t`). Nunca vacío: si falta la clave,
    /// el core devuelve la propia clave.
    pub fn t(&self, key: &str) -> String {
        self.i18n.t(key).to_string()
    }

    /// El tema activo, resuelto del catálogo por el id de los settings (cae al default si el
    /// id no existe).
    // Lo consume `theme_apply` (volcar colores al global Theme) en la tarea de temas.
    pub fn active_theme(&self) -> &Theme {
        self.themes.get(&self.settings.theme)
    }

    /// Persiste los settings actuales (escritura chica de JSON).
    pub fn save(&self) {
        config::save_settings(&self.config_dir, &self.settings);
    }

    /// Cambia el idioma activo y persiste. Devuelve `true` si efectivamente cambió (el core
    /// rechaza un idioma que no existe en el catálogo).
    // Lo consume el combo de idioma de la ventana de configuración (cambio en caliente).
    pub fn set_language(&mut self, lang: LangId) -> bool {
        let changed = self.i18n.set_language(&lang);
        if changed {
            self.settings.language = lang;
            self.save();
        }
        changed
    }

    /// Cambia el tema activo y persiste.
    pub fn set_theme(&mut self, id: ThemeId) {
        self.settings.theme = id;
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carga_defaults_en_dir_vacio() {
        let tmp = tempfile::tempdir().unwrap();
        let c = ConfigCtrl::new(tmp.path().to_path_buf());
        // `t` nunca vacío: una clave inexistente cae a sí misma.
        assert!(!c.t("clave.inexistente").is_empty());
        // 4 temas embebidos como mínimo.
        assert!(c.themes.available().len() >= 4);
    }

    #[test]
    fn set_theme_persiste_y_recarga() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        {
            let mut c = ConfigCtrl::new(dir.clone());
            c.set_theme(ThemeId::new("light"));
            assert_eq!(c.settings.theme, ThemeId::new("light"));
        }
        // Reabrir: el tema persistió.
        let c2 = ConfigCtrl::new(dir);
        assert_eq!(c2.settings.theme, ThemeId::new("light"));
    }
}
