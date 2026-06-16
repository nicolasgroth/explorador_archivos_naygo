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

use naygo_core::config::{self, BarPosition, OpsMode, Settings};
use naygo_core::i18n::{I18n, LangId};
use naygo_core::keymap::{Action, Chord, KeyCode, KeyMap};
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
    /// directorio vacío o archivos corruptos caen a defaults sin panic. En el PRIMER arranque
    /// (no hay settings.json) detecta el idioma del SO y lo deja persistido.
    pub fn new(config_dir: PathBuf) -> ConfigCtrl {
        let first_launch = !config_dir.join("settings.json").exists();
        let (mut settings, _recovered) = config::load_settings_flagged(&config_dir);
        let mut i18n = I18n::load(&config_dir, &settings.language);
        if first_launch {
            // Elegir el idioma según el locale del SO (es-CL → es, en-US → en, …) entre los
            // disponibles; persistirlo para que el próximo arranque no vuelva a detectar.
            if let Some(loc) = naygo_platform::locale::os_locale() {
                let lang = naygo_core::i18n::pick_default_language(&loc, i18n.available());
                if i18n.set_language(&lang) {
                    settings.language = lang;
                    config::save_settings(&config_dir, &settings);
                }
            }
        }
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

    // --- Setters de ajustes (cada uno persiste). Los usa la ventana de configuración. ---

    /// Modo de ejecución de operaciones: 0 = cola, 1 = paralelo.
    pub fn set_ops_mode(&mut self, mode: i32) {
        self.settings.ops_mode = if mode == 1 {
            OpsMode::Parallel
        } else {
            OpsMode::Queue
        };
        self.save();
    }

    /// Confirmar también el borrado a papelera.
    pub fn set_confirm_trash(&mut self, v: bool) {
        self.settings.confirm_trash = v;
        self.save();
    }

    /// Mostrar el resumen al terminar una operación.
    pub fn set_show_op_summary(&mut self, v: bool) {
        self.settings.show_op_summary = v;
        self.save();
    }

    /// Mostrar la fila virtual ".." al tope de los paneles.
    pub fn set_show_parent(&mut self, v: bool) {
        self.settings.show_parent_entry = v;
        self.save();
    }

    /// Botones de la barra solo con ícono.
    pub fn set_icon_only(&mut self, v: bool) {
        self.settings.icon_only = v;
        self.save();
    }

    /// Posición de la barra: 0 = arriba, 1 = al costado.
    pub fn set_bar_position(&mut self, pos: i32) {
        self.settings.bar_position = if pos == 1 {
            BarPosition::Side
        } else {
            BarPosition::Top
        };
        self.save();
    }

    /// Al calcular tamaño de carpeta, no bajar a subdirectorios.
    pub fn set_size_no_subdirs(&mut self, v: bool) {
        self.settings.size_no_subdirs = v;
        self.save();
    }

    /// Pegar texto/imagen: pedir confirmación de nombre (modo B).
    pub fn set_paste_confirm(&mut self, v: bool) {
        self.settings.paste_confirm = v;
        self.save();
    }

    /// Iniciar Naygo con Windows: escribe/borra la entrada Run del registro y persiste el
    /// ajuste. Si el registro falla (permiso), no cambia el ajuste (queda como estaba).
    pub fn set_autostart(&mut self, on: bool) {
        if naygo_platform::autostart::set_enabled(on).is_ok() {
            self.settings.autostart = on;
            self.save();
        }
    }

    /// Formato de fecha de las columnas: 0=ISO aaaa-mm-dd hh:mm, 1=ISO solo fecha,
    /// 2=dd-mm-aaaa hh:mm, 3=dd-mm-aaaa solo fecha.
    pub fn set_date_format(&mut self, idx: i32) {
        use naygo_core::format::DateFormat::*;
        self.settings.date_format = match idx {
            1 => IsoDate,
            2 => DmyMinute,
            3 => DmyDate,
            _ => IsoMinute,
        };
        self.save();
    }

    /// Plantilla de nombre para texto pegado.
    pub fn set_paste_text_name(&mut self, name: String) {
        self.settings.paste_text_name = name;
        self.save();
    }

    /// Extensión (sin punto) para texto pegado.
    pub fn set_paste_text_ext(&mut self, ext: String) {
        self.settings.paste_text_ext = ext;
        self.save();
    }

    // --- Editor de atajos ---

    /// Texto legible de un chord, p. ej. "Ctrl+C", "Supr", "Shift+↑". Sin i18n: los nombres de
    /// modificadores y teclas son convención.
    pub fn chord_to_text(chord: &Chord) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if chord.ctrl {
            parts.push("Ctrl");
        }
        if chord.shift {
            parts.push("Shift");
        }
        if chord.alt {
            parts.push("Alt");
        }
        let key = match chord.key {
            KeyCode::ArrowUp => "↑".to_string(),
            KeyCode::ArrowDown => "↓".to_string(),
            KeyCode::ArrowLeft => "←".to_string(),
            KeyCode::ArrowRight => "→".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::Escape => "Esc".to_string(),
            KeyCode::Delete => "Supr".to_string(),
            KeyCode::F2 => "F2".to_string(),
            KeyCode::F3 => "F3".to_string(),
            KeyCode::F4 => "F4".to_string(),
            KeyCode::F5 => "F5".to_string(),
            KeyCode::F6 => "F6".to_string(),
            KeyCode::PageDown => "PageDown".to_string(),
            KeyCode::PageUp => "PageUp".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::Space => "Espacio".to_string(),
            KeyCode::Char(c) => c.to_uppercase().to_string(),
        };
        parts.push(&key);
        parts.join("+")
    }

    /// El primer chord de una acción como texto, o vacío si no tiene atajo.
    pub fn chord_text_for(&self, action: Action) -> String {
        self.keymap
            .chords_for(action)
            .first()
            .map(Self::chord_to_text)
            .unwrap_or_default()
    }

    /// Reasigna el PRIMER chord de `action` a `chord`. Si el chord ya pertenecía a otra acción,
    /// se la quita (la reasignación gana) y devuelve esa acción desplazada (para avisar del
    /// conflicto). Persiste el keymap.
    pub fn rebind(&mut self, action: Action, chord: Chord) -> Option<Action> {
        // Quitar el chord viejo de esta acción (reemplazo del primero), luego asignar el nuevo.
        let old: Vec<Chord> = self.keymap.chords_for(action).to_vec();
        for c in old {
            self.keymap.unbind(action, &c);
        }
        let displaced = self.keymap.bind(action, chord);
        config::save_keymap(&self.config_dir, &self.keymap);
        displaced
    }

    /// Restaura los atajos por defecto de una acción y persiste.
    pub fn reset_shortcut(&mut self, action: Action) {
        self.keymap.reset_action(action);
        config::save_keymap(&self.config_dir, &self.keymap);
    }

    /// Restaura TODOS los atajos a sus valores por defecto y persiste.
    pub fn reset_all_shortcuts(&mut self) {
        self.keymap.reset_all();
        config::save_keymap(&self.config_dir, &self.keymap);
    }

    /// Clave estable de una acción (el nombre de la variante, p. ej. "Copy"). Sirve de
    /// identificador en la UI para round-trip Slint→Rust sin acoplar a un índice.
    pub fn action_key(action: Action) -> String {
        // Una enum de variantes unitarias serializa al nombre de la variante.
        match serde_json::to_value(action) {
            Ok(serde_json::Value::String(s)) => s,
            _ => String::new(),
        }
    }

    /// La acción para una clave estable (inversa de `action_key`).
    pub fn action_from_key(key: &str) -> Option<Action> {
        serde_json::from_value(serde_json::Value::String(key.to_string())).ok()
    }

    /// Lista de atajos para el editor: (clave estable, nombre legible, chord como texto), en el
    /// orden de presentación de `Action::all()`.
    pub fn shortcut_list(&self) -> Vec<(String, String, String)> {
        Action::all()
            .iter()
            .map(|&a| {
                (
                    Self::action_key(a),
                    self.t(a.i18n_key()),
                    self.chord_text_for(a),
                )
            })
            .collect()
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

    #[test]
    fn setters_persisten() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        {
            let mut c = ConfigCtrl::new(dir.clone());
            c.set_ops_mode(1);
            c.set_confirm_trash(true);
            c.set_paste_text_ext("md".into());
        }
        let c2 = ConfigCtrl::new(dir);
        assert_eq!(c2.settings.ops_mode, OpsMode::Parallel);
        assert!(c2.settings.confirm_trash);
        assert_eq!(c2.settings.paste_text_ext, "md");
    }

    #[test]
    fn chord_to_text_legible() {
        assert_eq!(
            ConfigCtrl::chord_to_text(&Chord::ctrl(KeyCode::Char('c'))),
            "Ctrl+C"
        );
        assert_eq!(
            ConfigCtrl::chord_to_text(&Chord::plain(KeyCode::Delete)),
            "Supr"
        );
        assert_eq!(
            ConfigCtrl::chord_to_text(&Chord::shift(KeyCode::ArrowUp)),
            "Shift+↑"
        );
    }

    #[test]
    fn action_key_round_trip() {
        let k = ConfigCtrl::action_key(Action::Copy);
        assert_eq!(k, "Copy");
        assert_eq!(ConfigCtrl::action_from_key(&k), Some(Action::Copy));
        assert_eq!(ConfigCtrl::action_from_key("noexiste"), None);
    }

    #[test]
    fn shortcut_list_lista_todas_las_acciones() {
        let tmp = tempfile::tempdir().unwrap();
        let c = ConfigCtrl::new(tmp.path().to_path_buf());
        let rows = c.shortcut_list();
        assert_eq!(rows.len(), Action::all().len());
        // Copy debe estar y traer su chord por defecto (Ctrl+C).
        let copy = rows.iter().find(|(k, _, _)| k == "Copy").unwrap();
        assert_eq!(copy.2, "Ctrl+C");
    }

    #[test]
    fn rebind_detecta_conflicto_y_reset() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = ConfigCtrl::new(tmp.path().to_path_buf());
        // Ctrl+C es Copy por defecto. Reasignarlo a Cut → conflicto con Copy.
        let displaced = c.rebind(Action::Cut, Chord::ctrl(KeyCode::Char('c')));
        assert_eq!(displaced, Some(Action::Copy));
        assert_eq!(
            c.keymap.action_for(&Chord::ctrl(KeyCode::Char('c'))),
            Some(Action::Cut)
        );
        // Reset de Cut vuelve Ctrl+C a Copy (default de Copy lo recupera tras reset de Copy).
        c.reset_shortcut(Action::Copy);
        assert_eq!(
            c.keymap.action_for(&Chord::ctrl(KeyCode::Char('c'))),
            Some(Action::Copy)
        );
    }
}
