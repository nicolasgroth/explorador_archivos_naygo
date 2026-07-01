// Naygo — controlador de configuración de la UI Slint (Fase 4). Posee los Settings, el
// catálogo i18n, el catálogo de temas y el mapa de atajos, todo cargado desde el core, y los
// persiste en el directorio portable. La UI (ventana de config, editor de atajos) habla con
// este controlador; nunca toca el disco directo.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// API foundacional de la Fase 4: este módulo se introduce completo, pero sus consumidores
// (theme_apply, i18n_keys, la ventana de configuración y el editor de atajos) se cablean en
// las tareas siguientes de la misma fase. Hasta entonces algunos métodos quedan sin llamador
// desde el binario, de ahí el allow a nivel de módulo. Se quita al cerrar la fase.
#![allow(dead_code)]

use naygo_core::config::{self, BarPosition, OpsMode, Settings};
use naygo_core::i18n::{I18n, LangId};
use naygo_core::keymap::{Action, Chord, KeyCode, KeyMap};
use naygo_core::theme::{
    is_builtin_id, theme_slug, Theme, ThemeBase, ThemeCatalog, ThemeColor, ThemeId,
};
use std::path::PathBuf;

/// Cantidad de tokens de color editables de un tema (orden fijo, ver `set_token_color`).
pub const THEME_TOKEN_COUNT: usize = 12;

/// Estado del tema que se está editando (duplicado de un builtin o de uno de usuario). El editor
/// muta este `Theme` en memoria, aplica el preview en vivo, y al guardar lo escribe a disco.
struct ThemeEditState {
    /// El tema en construcción (sus 11 tokens + nombre + base se editan en vivo).
    theme: Theme,
    /// El tema activo ANTES de entrar al editor; al cancelar se re-aplica para revertir el preview.
    prev_theme_id: ThemeId,
    /// Si se duplicó de un builtin, su id (para "Restaurar de fábrica"). `None` si nació de un
    /// tema de usuario (no hay fábrica a la que volver).
    src_builtin_id: Option<String>,
    /// El id del tema de USUARIO que se está editando IN-PLACE (botón "Editar"): al guardar se
    /// sobrescribe ese mismo `<id>.json` en vez de crear uno nuevo. `None` cuando el tema nació de
    /// un duplicado (builtin o usuario), en cuyo caso el guardado asigna un id-slug nuevo y único.
    origin_id: Option<ThemeId>,
}

/// Estado de configuración de la app: ajustes + idioma + temas + atajos.
pub struct ConfigCtrl {
    pub settings: Settings,
    pub i18n: I18n,
    pub themes: ThemeCatalog,
    pub keymap: KeyMap,
    pub config_dir: PathBuf,
    /// Tema en edición (editor de temas). `None` mientras no haya un editor abierto.
    editing: Option<ThemeEditState>,
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
            editing: None,
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

    /// Cambia el tema activo SOLO en memoria (NO persiste): la elección dura lo que dura la
    /// sesión y no toca settings.json. La usa `main` para el argumento de CLI `--theme`, que
    /// pinta un tema por una sola ejecución sin cambiar el tema guardado del usuario.
    /// Devuelve `true` si el id existe en el catálogo (si no, no toca nada).
    pub fn set_theme_ephemeral(&mut self, id: ThemeId) -> bool {
        if !self
            .themes
            .available()
            .iter()
            .any(|t| t.as_str() == id.as_str())
        {
            return false;
        }
        self.settings.theme = id;
        true
    }

    // --- Editor de temas (galería de Apariencia) ---
    //
    // Los 12 tokens de color de un `Theme` se exponen por un índice estable 0..12. El ORDEN ES
    // FIJO y debe coincidir entre `editing_token_hex` y `set_token_color` (y con la lista de la
    // UI): 0=accent, 1=panel_bg, 2=row_bg, 3=row_alt_bg, 4=text, 5=text_dim, 6=selection_bg,
    // 7=active_bar, 8=error, 9=highlight, 10=border, 11=row_inactive_bg.

    /// Lee el token `idx` (0..12) de un tema. Fuera de rango → `accent` (defensivo, nunca panic).
    fn token_of(theme: &Theme, idx: usize) -> ThemeColor {
        match idx {
            0 => theme.accent,
            1 => theme.panel_bg,
            2 => theme.row_bg,
            3 => theme.row_alt_bg,
            4 => theme.text,
            5 => theme.text_dim,
            6 => theme.selection_bg,
            7 => theme.active_bar,
            8 => theme.error,
            9 => theme.highlight,
            10 => theme.border,
            11 => theme.row_inactive_bg,
            _ => theme.accent,
        }
    }

    /// Escribe el token `idx` (0..12) de un tema. Fuera de rango → no hace nada.
    fn set_token_of(theme: &mut Theme, idx: usize, c: ThemeColor) {
        match idx {
            0 => theme.accent = c,
            1 => theme.panel_bg = c,
            2 => theme.row_bg = c,
            3 => theme.row_alt_bg = c,
            4 => theme.text = c,
            5 => theme.text_dim = c,
            6 => theme.selection_bg = c,
            7 => theme.active_bar = c,
            8 => theme.error = c,
            9 => theme.highlight = c,
            10 => theme.border = c,
            11 => theme.row_inactive_bg = c,
            _ => {}
        }
    }

    /// Genera un id-slug único a partir de `name`: el slug base, y si ya existe en el catálogo,
    /// le agrega un sufijo -2, -3, … hasta encontrar uno libre.
    fn unique_slug(&self, name: &str) -> String {
        let base = theme_slug(name);
        let taken = |id: &str| self.themes.available().iter().any(|t| t.as_str() == id);
        if !taken(&base) {
            return base;
        }
        let mut n = 2;
        loop {
            let cand = format!("{base}-{n}");
            if !taken(&cand) {
                return cand;
            }
            n += 1;
        }
    }

    /// Duplica el tema `src_id` para entrar al editor: clona su paleta, le pone nombre
    /// "<nombre> (copia)" y un id-slug único, recuerda el tema activo actual (para revertir al
    /// cancelar) y, si `src_id` es de fábrica, lo recuerda para "Restaurar de fábrica". NO guarda
    /// nada todavía: el tema queda "en edición". Devuelve el id nuevo (editable).
    pub fn duplicate_theme(&mut self, src_id: &str) -> String {
        let src = ThemeId::new(src_id);
        let mut theme = self.themes.get(&src).clone();
        // Sufijo traducido ("(copia)" / "(copy)"): nada hardcoded para que el nombre del duplicado
        // salga en el idioma activo (en inglés no debe quedar "Dark Blue (copia)").
        theme.name = format!("{} {}", theme.name, self.t("slint.theme.copy_suffix"));
        let new_id = self.unique_slug(&theme.name);
        let src_builtin_id = if is_builtin_id(src_id) {
            Some(src_id.to_string())
        } else {
            None
        };
        self.editing = Some(ThemeEditState {
            theme,
            prev_theme_id: self.settings.theme.clone(),
            src_builtin_id,
            // Un duplicado siempre nace como tema nuevo: el guardado le asigna un id-slug único.
            origin_id: None,
        });
        new_id
    }

    /// Abre el editor sobre un tema de USUARIO existente (botón "Editar"): carga su paleta tal
    /// cual para editarla en sitio (mismo id, se sobrescribe al guardar). Para builtins no hace
    /// nada (esos se duplican, no se editan). Devuelve el id editado, o vacío si no aplica.
    pub fn edit_user_theme(&mut self, id: &str) -> String {
        if is_builtin_id(id) {
            return String::new();
        }
        let tid = ThemeId::new(id);
        if !self.themes.available().iter().any(|t| t.as_str() == id) {
            return String::new();
        }
        let theme = self.themes.get(&tid).clone();
        self.editing = Some(ThemeEditState {
            theme,
            prev_theme_id: self.settings.theme.clone(),
            src_builtin_id: None,
            // Edición in-place: al guardar se sobrescribe ESTE id (no se crea un .json nuevo).
            origin_id: Some(tid),
        });
        id.to_string()
    }

    /// ¿Hay un tema en edición ahora mismo?
    pub fn is_editing_theme(&self) -> bool {
        self.editing.is_some()
    }

    /// Nombre del tema en edición (vacío si no hay editor abierto).
    pub fn editing_name(&self) -> String {
        self.editing
            .as_ref()
            .map(|e| e.theme.name.clone())
            .unwrap_or_default()
    }

    /// Cambia el nombre del tema en edición (no toca disco).
    pub fn set_editing_name(&mut self, name: String) {
        if let Some(e) = self.editing.as_mut() {
            e.theme.name = name;
        }
    }

    /// Base del tema en edición como índice de combo: 0=Oscuro, 1=Claro. Sin editor → 0.
    pub fn editing_base_index(&self) -> i32 {
        match self.editing.as_ref().map(|e| e.theme.base) {
            Some(ThemeBase::Light) => 1,
            _ => 0,
        }
    }

    /// Fija la base del tema en edición desde un índice de combo (1=Claro, resto=Oscuro). La base
    /// solo decide cómo se rellenan los campos faltantes al deserializar; no re-aplica preview.
    pub fn set_editing_base(&mut self, idx: i32) {
        if let Some(e) = self.editing.as_mut() {
            e.theme.base = if idx == 1 {
                ThemeBase::Light
            } else {
                ThemeBase::Dark
            };
        }
    }

    /// Hex "#rrggbb" del token `idx` (0..12) del tema en edición. Sin editor → "#000000".
    pub fn editing_token_hex(&self, idx: usize) -> String {
        self.editing
            .as_ref()
            .map(|e| Self::token_of(&e.theme, idx).to_hex())
            .unwrap_or_else(|| "#000000".to_string())
    }

    /// Componentes (r, g, b) del token `idx` del tema en edición. Útil para inicializar el picker
    /// (que toma r/g/b enteros, no parsea hex). Sin editor → (0, 0, 0).
    pub fn editing_token_rgb(&self, idx: usize) -> (u8, u8, u8) {
        self.editing
            .as_ref()
            .map(|e| {
                let c = Self::token_of(&e.theme, idx);
                (c.r, c.g, c.b)
            })
            .unwrap_or((0, 0, 0))
    }

    /// Fija el token `idx` (0..11) del tema en edición desde un hex "#rrggbb". Si el hex es
    /// inválido o no hay editor, no hace nada. El llamador (main) re-aplica el preview en vivo
    /// leyendo `editing_theme()` tras esta llamada.
    pub fn set_token_color(&mut self, idx: usize, hex: &str) {
        if let Some(c) = ThemeColor::from_hex(hex) {
            if let Some(e) = self.editing.as_mut() {
                Self::set_token_of(&mut e.theme, idx, c);
            }
        }
    }

    /// ¿El tema en edición tiene activado "paneles inactivos planos"? Sin editor → `false`.
    pub fn editing_flat_inactive(&self) -> bool {
        self.editing
            .as_ref()
            .map(|e| e.theme.flat_inactive_panels)
            .unwrap_or(false)
    }

    /// Fija "paneles inactivos planos" del tema en edición. Si no hay editor, no hace nada. El
    /// llamador re-aplica el preview en vivo leyendo `editing_theme()` tras esta llamada.
    pub fn set_editing_flat_inactive(&mut self, v: bool) {
        if let Some(e) = self.editing.as_mut() {
            e.theme.flat_inactive_panels = v;
        }
    }

    /// El tema en edición (para aplicar el preview en vivo con `theme_apply::apply`). `None` si no
    /// hay editor abierto.
    pub fn editing_theme(&self) -> Option<&Theme> {
        self.editing.as_ref().map(|e| &e.theme)
    }

    /// Guarda el tema en edición: lo serializa a `<config>/themes/<slug>.json`, recarga el
    /// catálogo, lo deja activo y persiste settings. Devuelve el id guardado, o `None` si no había
    /// editor o falló la escritura. Limpia el estado de edición.
    pub fn save_editing_theme(&mut self) -> Option<ThemeId> {
        let state = self.editing.take()?;
        let theme = state.theme;
        // Resolver el id de destino según el origen del editor:
        //  - Edición IN-PLACE de un tema de usuario (botón "Editar"): se sobrescribe ese mismo id.
        //    Si el usuario cambió el nombre y el slug ya no coincide, se asigna un slug nuevo
        //    (único, sin pisar a otro tema) y se borra el `<id viejo>.json` para no dejar huérfanos.
        //  - Tema nuevo (duplicado): siempre un id-slug único.
        let new_slug = theme_slug(&theme.name);
        let (id, old_to_delete): (String, Option<String>) = match &state.origin_id {
            Some(origin) if origin.as_str() == new_slug => {
                // Mismo slug → sobrescribir en sitio, sin borrar nada.
                (origin.as_str().to_string(), None)
            }
            Some(origin) => {
                // Renombrado: id nuevo único + borrar el .json del id viejo.
                (
                    self.unique_slug(&theme.name),
                    Some(origin.as_str().to_string()),
                )
            }
            None => (self.unique_slug(&theme.name), None),
        };
        let theme_dir = self.config_dir.join("themes");
        if std::fs::create_dir_all(&theme_dir).is_err() {
            return None;
        }
        let path = theme_dir.join(format!("{id}.json"));
        let json = serde_json::to_string_pretty(&theme).ok()?;
        if std::fs::write(&path, json).is_err() {
            return None;
        }
        // Borrar el .json del id viejo tras escribir el nuevo (ignorando el error, igual que
        // `delete_user_theme` con `.is_ok()`: si ya no está, no pasa nada).
        if let Some(old) = old_to_delete {
            let _ = std::fs::remove_file(theme_dir.join(format!("{old}.json")));
        }
        let new_id = ThemeId::new(&id);
        self.themes = ThemeCatalog::load(&self.config_dir, &new_id);
        self.settings.theme = new_id.clone();
        self.save();
        Some(new_id)
    }

    /// Cancela el editor: descarta el tema en edición y devuelve el id del tema que estaba activo
    /// antes (para que el llamador re-aplique ese tema y revierta el preview). `None` si no había
    /// editor abierto.
    pub fn cancel_editing(&mut self) -> Option<ThemeId> {
        self.editing.take().map(|e| e.prev_theme_id)
    }

    /// Restaura los 12 tokens del tema en edición a los del builtin del que se duplicó (botón
    /// "Restaurar de fábrica"), incluyendo "paneles inactivos planos". Conserva el nombre/base
    /// actuales del editor. No hace nada si el tema en edición no proviene de un builtin. El
    /// llamador re-aplica el preview tras esto.
    pub fn restore_factory_editing(&mut self) {
        let Some(state) = self.editing.as_mut() else {
            return;
        };
        let Some(src_id) = state.src_builtin_id.clone() else {
            return;
        };
        let factory = self.themes.get(&ThemeId::new(&src_id)).clone();
        let e = self.editing.as_mut().expect("editing recién comprobado");
        e.theme.accent = factory.accent;
        e.theme.panel_bg = factory.panel_bg;
        e.theme.row_bg = factory.row_bg;
        e.theme.row_alt_bg = factory.row_alt_bg;
        e.theme.row_inactive_bg = factory.row_inactive_bg;
        e.theme.text = factory.text;
        e.theme.text_dim = factory.text_dim;
        e.theme.selection_bg = factory.selection_bg;
        e.theme.active_bar = factory.active_bar;
        e.theme.error = factory.error;
        e.theme.highlight = factory.highlight;
        e.theme.border = factory.border;
        e.theme.flat_inactive_panels = factory.flat_inactive_panels;
        e.theme.base = factory.base;
    }

    /// Borra un tema de USUARIO: elimina `<config>/themes/<id>.json`, recarga el catálogo, y si era
    /// el activo cae al default. Para builtins no hace nada (no son borrables). Devuelve `true` si
    /// borró algo.
    pub fn delete_user_theme(&mut self, id: &str) -> bool {
        if is_builtin_id(id) {
            return false;
        }
        let path = self.config_dir.join("themes").join(format!("{id}.json"));
        let removed = std::fs::remove_file(&path).is_ok();
        // Recargar el catálogo (refleja el borrado) y, si el borrado era el activo, caer al default.
        let active = self.settings.theme.clone();
        self.themes = ThemeCatalog::load(&self.config_dir, &active);
        if active.as_str() == id {
            self.settings.theme = ThemeCatalog::default_id();
            self.save();
        }
        removed
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

    /// Preguntar al arrastrar archivos/carpetas entre paneles antes de copiar/mover.
    pub fn set_confirm_drop_between_panes(&mut self, v: bool) {
        self.settings.confirm_drop_between_panes = v;
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
    /// ajuste. Si el registro falla (permiso), no cambia el ajuste (queda como estaba). Cuando
    /// `autostart_minimized` está activo, la entrada Run incluye `--tray` para que el proceso
    /// arranque directo a la bandeja (ver `set_autostart_minimized` y main.rs).
    pub fn set_autostart(&mut self, on: bool) {
        let args: &[&str] = if self.settings.autostart_minimized {
            &["--tray"]
        } else {
            &[]
        };
        if naygo_platform::autostart::set_enabled(on, args).is_ok() {
            self.settings.autostart = on;
            self.save();
        }
    }

    /// Al iniciar con Windows, arrancar minimizado en la bandeja. Si autostart ya está activo,
    /// reescribe la entrada Run con/sin --tray para que el cambio surta efecto de inmediato.
    /// El flag SIEMPRE se persiste; la reescritura del registro es best-effort (si falla por
    /// permisos, se loguea y el sufijo --tray se corrige en el próximo toggle de autostart).
    pub fn set_autostart_minimized(&mut self, v: bool) {
        self.settings.autostart_minimized = v;
        self.save();
        if self.settings.autostart {
            let args: &[&str] = if v { &["--tray"] } else { &[] };
            if let Err(e) = naygo_platform::autostart::set_enabled(true, args) {
                crate::logging::log_line(&format!(
                    "no se pudo reescribir la entrada Run con --tray: {e}"
                ));
            }
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

    /// Set de íconos por id (de los disponibles en el catálogo: embebidos + packs sueltos).
    /// Lo coacciona contra el catálogo (un id inexistente cae a "lucide") y persiste.
    pub fn set_icon_set(&mut self, id: String) {
        let catalog = naygo_core::icon_set::IconSetCatalog::load(&self.config_dir);
        self.settings.icon_set = catalog.resolve(&id);
        self.save();
    }

    /// Densidad de las filas: 0=Compacta (22px), 1=Cómoda (26px).
    pub fn set_row_density(&mut self, idx: i32) {
        use naygo_core::config::RowDensity::*;
        self.settings.row_density = if idx == 1 { Comfortable } else { Compact };
        self.save();
    }

    /// Formato de la columna de tamaño: 0=Auto legible, 1=Bytes (miles), 2=KB, 3=MB.
    pub fn set_size_format(&mut self, idx: i32) {
        use naygo_core::format::SizeFormat::*;
        self.settings.size_format = match idx {
            1 => Bytes,
            2 => Kb,
            3 => Mb,
            _ => Auto,
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

    // --- Avanzado ---

    /// Cómo se muestra el progreso de operaciones: 0=Panel, 1=Modal, 2=Siempre visible.
    pub fn set_ops_display(&mut self, idx: i32) {
        use naygo_core::config::OpsDisplay::*;
        self.settings.ops_display = match idx {
            1 => Modal,
            2 => AlwaysVisible,
            _ => Panel,
        };
        self.save();
    }

    /// Formato de salida para imagen pegada: 0=PNG, 1=JPG.
    pub fn set_paste_image_fmt(&mut self, idx: i32) {
        use naygo_core::clipboard::ImageFmt::*;
        self.settings.paste_image_fmt = if idx == 1 { Jpg } else { Png };
        self.save();
    }

    /// Plantilla de nombre para imagen pegada.
    pub fn set_paste_image_name(&mut self, name: String) {
        self.settings.paste_image_name = name;
        self.save();
    }

    /// Calidad de salida JPG para imagen pegada (1–100; el codificador no acepta 0).
    pub fn set_paste_jpg_quality(&mut self, quality: u8) {
        self.settings.paste_jpg_quality = quality.clamp(1, 100);
        self.save();
    }

    /// Mostrar el ícono de Naygo en la bandeja del sistema.
    pub fn set_tray_enabled(&mut self, v: bool) {
        self.settings.tray_enabled = v;
        self.save();
    }

    /// Al cerrar la ventana, ocultar a la bandeja en vez de salir.
    pub fn set_close_to_tray(&mut self, v: bool) {
        self.settings.close_to_tray = v;
        self.save();
    }

    /// Activa/desactiva el hotkey global y persiste. El (re)registro real lo hace la UI (main),
    /// que tiene el `GlobalHotkey` vivo; aquí solo se persiste el flag.
    pub fn set_global_hotkey_enabled(&mut self, on: bool) {
        self.settings.global_hotkey_enabled = on;
        self.save();
    }

    /// Cambia la combinación del hotkey global y persiste. Valida ≥1 modificador (un hotkey
    /// global de una sola tecla es inaceptable). Devuelve Err si es inválida; el re-registro y el
    /// aviso de rechazo del SO los maneja la UI.
    pub fn set_global_hotkey(&mut self, chord: Chord) -> Result<(), String> {
        if !(chord.ctrl || chord.alt || chord.shift) {
            return Err("el atajo global necesita al menos un modificador (Ctrl/Alt/Shift)".into());
        }
        self.settings.global_hotkey = chord;
        self.save();
        Ok(())
    }

    /// Agrupar los archivos recién aparecidos al final del listado (en vez de insertarlos
    /// ya ordenados).
    pub fn set_new_items_at_end(&mut self, v: bool) {
        self.settings.new_items_at_end = v;
        self.save();
    }

    /// Modo de bajo consumo: 0=Auto, 1=Siempre, 2=Nunca.
    pub fn set_low_power_mode(&mut self, idx: i32) {
        use naygo_core::config::LowPowerMode::*;
        self.settings.low_power_mode = match idx {
            1 => Always,
            2 => Never,
            _ => Auto,
        };
        self.save();
    }

    /// Resaltar automáticamente el código de extensiones conocidas en el modo Auto del preview.
    pub fn auto_highlight_code(&self) -> bool {
        self.settings.auto_highlight_code
    }

    /// Activa/desactiva el auto-resaltado de código y persiste.
    pub fn set_auto_highlight_code(&mut self, v: bool) {
        self.settings.auto_highlight_code = v;
        self.save();
    }

    // --- Visibilidad de archivos (menú del "ojo" en la toolbar) ---

    /// ¿Mostrar los archivos/carpetas con atributo OCULTO?
    pub fn show_hidden(&self) -> bool {
        self.settings.show_hidden
    }

    /// Alterna mostrar ocultos y persiste. El llamador re-arma las vistas (refresco al instante).
    pub fn set_show_hidden(&mut self, v: bool) {
        self.settings.show_hidden = v;
        self.save();
    }

    /// ¿Mostrar los archivos/carpetas con atributo de SISTEMA?
    pub fn show_system(&self) -> bool {
        self.settings.show_system
    }

    /// Alterna mostrar de sistema y persiste. El llamador re-arma las vistas.
    pub fn set_show_system(&mut self, v: bool) {
        self.settings.show_system = v;
        self.save();
    }

    /// ¿Esconder los archivos/carpetas cuyo nombre empieza con punto?
    pub fn hide_dotfiles(&self) -> bool {
        self.settings.hide_dotfiles
    }

    /// Alterna esconder dotfiles y persiste. El llamador re-arma las vistas.
    pub fn set_hide_dotfiles(&mut self, v: bool) {
        self.settings.hide_dotfiles = v;
        self.save();
    }

    // --- Pie de panel (footer) ---

    /// ¿Mostrar el pie (barra inferior) en cada panel de archivos?
    pub fn footer_enabled(&self) -> bool {
        self.settings.footer_enabled
    }

    /// Activa/desactiva el footer y persiste.
    pub fn set_footer_enabled(&mut self, v: bool) {
        self.settings.footer_enabled = v;
        self.save();
    }

    /// Índice del preset del footer para el combo: 0=Compacta, 1=Completa, 2=Solo disco,
    /// 3=Solo selección, 4=Personalizada.
    pub fn footer_preset_index(&self) -> i32 {
        use naygo_core::footer::FooterPreset::*;
        match self.settings.footer_preset {
            Compact => 0,
            Full => 1,
            DiskOnly => 2,
            SelectionOnly => 3,
            Custom(_) => 4,
        }
    }

    /// Aplica el índice del combo al preset del footer (inversa de `footer_preset_index`) y
    /// persiste. Al pasar a Personalizada (4) conserva el template guardado en `footer_custom_template`.
    pub fn set_footer_preset_index(&mut self, idx: i32) {
        use naygo_core::footer::FooterPreset::*;
        self.settings.footer_preset = match idx {
            1 => Full,
            2 => DiskOnly,
            3 => SelectionOnly,
            4 => Custom(self.settings.footer_custom_template.clone()),
            _ => Compact,
        };
        self.save();
    }

    /// Template personalizado del footer (cuando el preset es Personalizada).
    pub fn footer_custom_template(&self) -> String {
        self.settings.footer_custom_template.clone()
    }

    /// Cambia el template personalizado y persiste. Si el preset activo es Personalizada,
    /// lo mantiene en sincronía (para que el preview y el footer real usen el nuevo texto).
    pub fn set_footer_custom_template(&mut self, t: String) {
        self.settings.footer_custom_template = t.clone();
        if matches!(
            self.settings.footer_preset,
            naygo_core::footer::FooterPreset::Custom(_)
        ) {
            self.settings.footer_preset = naygo_core::footer::FooterPreset::Custom(t);
        }
        self.save();
    }

    /// Vista previa en vivo del footer: renderiza el preset/template activo con datos de
    /// ejemplo fijos (3 de 12 seleccionados, ~4,2 MB marcados, disco 112/500 GB). Útil para
    /// que el usuario vea el efecto del preset o de su template sin tener que mirar un panel.
    pub fn footer_preview(&self) -> String {
        use naygo_core::disk::DiskUsage;
        use naygo_core::footer::{render, FooterData};
        let data = FooterData {
            sel_count: 3,
            total_count: 12,
            marked_bytes: 4_404_019,
            disk: Some(DiskUsage {
                total: 500_000_000_000,
                free: 112_000_000_000,
            }),
            item_count: 12,
            file_count: 8,
            dir_count: 4,
        };
        render(
            &self.settings.footer_preset,
            &data,
            self.settings.size_format,
        )
    }

    // --- Carpeta de inicio (Home) ---

    /// Carpeta de inicio (botón Home). Vacío = carpeta personal del usuario.
    pub fn home_dir(&self) -> String {
        self.settings.home_dir.clone()
    }

    /// Cambia la carpeta de inicio y persiste. Vacío = carpeta personal del usuario.
    pub fn set_home_dir(&mut self, dir: String) {
        self.settings.home_dir = dir;
        self.save();
    }

    /// Restablece TODOS los ajustes a sus valores por defecto (factory reset) y persiste.
    /// El llamador debe reaplicar i18n/tema tras esto (cambian idioma/tema activos).
    pub fn factory_reset(&mut self) {
        self.settings = naygo_core::config::Settings::default();
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
            KeyCode::F1 => "F1".to_string(),
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
            c.set_theme(ThemeId::new("winxp"));
            assert_eq!(c.settings.theme, ThemeId::new("winxp"));
        }
        // Reabrir: el tema persistió.
        let c2 = ConfigCtrl::new(dir);
        assert_eq!(c2.settings.theme, ThemeId::new("winxp"));
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
    fn avanzado_setters_persisten() {
        use naygo_core::clipboard::ImageFmt;
        use naygo_core::config::{LowPowerMode, OpsDisplay};
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        {
            let mut c = ConfigCtrl::new(dir.clone());
            c.set_ops_display(2);
            c.set_paste_image_fmt(1);
            c.set_low_power_mode(1);
            c.set_new_items_at_end(true);
            c.set_tray_enabled(false);
            c.set_close_to_tray(true);
        }
        let c2 = ConfigCtrl::new(dir);
        assert_eq!(c2.settings.ops_display, OpsDisplay::AlwaysVisible);
        assert_eq!(c2.settings.paste_image_fmt, ImageFmt::Jpg);
        assert_eq!(c2.settings.low_power_mode, LowPowerMode::Always);
        assert!(c2.settings.new_items_at_end);
        assert!(!c2.settings.tray_enabled);
        assert!(c2.settings.close_to_tray);
    }

    #[test]
    fn global_hotkey_setters_persisten() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        {
            let mut c = ConfigCtrl::new(dir.clone());
            assert!(c.settings.global_hotkey_enabled, "activado de fábrica");
            c.set_global_hotkey_enabled(false);
            let chord = Chord::ctrl_alt(KeyCode::Char('w'));
            assert!(c.set_global_hotkey(chord).is_ok());
            assert_eq!(c.settings.global_hotkey, chord);
        }
        let c2 = ConfigCtrl::new(dir);
        assert!(!c2.settings.global_hotkey_enabled);
        assert_eq!(
            c2.settings.global_hotkey,
            Chord::ctrl_alt(KeyCode::Char('w'))
        );
    }

    #[test]
    fn global_hotkey_rechaza_sin_modificador() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = ConfigCtrl::new(tmp.path().to_path_buf());
        let before = c.settings.global_hotkey;
        let err = c.set_global_hotkey(Chord::plain(KeyCode::Char('q')));
        assert!(
            err.is_err(),
            "una sola tecla sin modificador debe rechazarse"
        );
        assert_eq!(
            c.settings.global_hotkey, before,
            "no debe mutar en el rechazo"
        );
    }

    #[test]
    fn visibilidad_setters_persisten() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        {
            let mut c = ConfigCtrl::new(dir.clone());
            // Defaults: mostrar todo, no esconder dotfiles.
            assert!(c.show_hidden());
            assert!(c.show_system());
            assert!(!c.hide_dotfiles());
            c.set_show_hidden(false);
            c.set_show_system(false);
            c.set_hide_dotfiles(true);
        }
        let c2 = ConfigCtrl::new(dir);
        assert!(!c2.show_hidden());
        assert!(!c2.show_system());
        assert!(c2.hide_dotfiles());
    }

    #[test]
    fn factory_reset_vuelve_a_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut c = ConfigCtrl::new(dir.clone());
        c.set_confirm_trash(true);
        c.set_ops_mode(1);
        c.factory_reset();
        let defaults = naygo_core::config::Settings::default();
        assert_eq!(c.settings, defaults, "factory reset = defaults exactos");
        // Y persistió: reabrir da los defaults.
        let c2 = ConfigCtrl::new(dir);
        assert_eq!(c2.settings, defaults);
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
    fn duplicar_builtin_entra_en_edicion_sin_guardar() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = ConfigCtrl::new(tmp.path().to_path_buf());
        let before = c.themes.available().len();
        let new_id = c.duplicate_theme("dark-blue");
        // El nuevo id es un slug nuevo (no colisiona) y aún NO está en el catálogo (no se guardó).
        assert!(!new_id.is_empty());
        assert!(c.is_editing_theme());
        assert_eq!(c.themes.available().len(), before, "no debe guardarse aún");
        // Nombre = "<orig> (copia)".
        assert!(c.editing_name().ends_with("(copia)"));
    }

    #[test]
    fn editar_token_cambia_el_tema_en_edicion() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = ConfigCtrl::new(tmp.path().to_path_buf());
        c.duplicate_theme("dark-blue");
        // accent = token 0.
        c.set_token_color(0, "#abcdef");
        assert_eq!(c.editing_token_hex(0), "#abcdef");
        assert_eq!(c.editing_token_rgb(0), (0xab, 0xcd, 0xef));
        // Hex inválido: no cambia nada.
        c.set_token_color(0, "no-es-hex");
        assert_eq!(c.editing_token_hex(0), "#abcdef");
        // El tema en edición refleja el cambio en su campo accent.
        assert_eq!(c.editing_theme().unwrap().accent.to_hex(), "#abcdef");
    }

    #[test]
    fn guardar_tema_escribe_json_y_lo_activa() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let saved_id;
        {
            let mut c = ConfigCtrl::new(dir.clone());
            c.duplicate_theme("dark-blue");
            c.set_editing_name("Mi Tema".into());
            c.set_token_color(0, "#112233"); // accent
            let id = c.save_editing_theme().expect("guarda");
            saved_id = id.as_str().to_string();
            assert!(!c.is_editing_theme(), "el editor se cierra al guardar");
            // Quedó activo y en el catálogo.
            assert_eq!(c.settings.theme.as_str(), saved_id);
            assert!(c.themes.available().iter().any(|t| t.as_str() == saved_id));
            // El archivo existe.
            assert!(dir.join("themes").join(format!("{saved_id}.json")).exists());
        }
        // Reabrir: el tema persiste con su color editado.
        let c2 = ConfigCtrl::new(dir);
        let t = c2.themes.get(&ThemeId::new(&saved_id));
        assert_eq!(t.accent.to_hex(), "#112233");
        assert_eq!(t.name, "Mi Tema");
    }

    #[test]
    fn cancelar_devuelve_el_tema_previo_y_descarta() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = ConfigCtrl::new(tmp.path().to_path_buf());
        c.set_theme(ThemeId::new("winxp"));
        c.duplicate_theme("dark-blue");
        c.set_token_color(0, "#ffffff");
        let prev = c.cancel_editing().expect("hay editor");
        assert_eq!(prev.as_str(), "winxp", "vuelve al tema previo");
        assert!(!c.is_editing_theme());
    }

    #[test]
    fn restaurar_de_fabrica_resetea_tokens() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = ConfigCtrl::new(tmp.path().to_path_buf());
        let orig_accent = c.themes.get(&ThemeId::new("dark-blue")).accent.to_hex();
        c.duplicate_theme("dark-blue");
        c.set_token_color(0, "#000000");
        assert_eq!(c.editing_token_hex(0), "#000000");
        c.restore_factory_editing();
        assert_eq!(
            c.editing_token_hex(0),
            orig_accent,
            "vuelve al accent de fábrica"
        );
    }

    #[test]
    fn borrar_tema_de_usuario_y_no_builtin() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut c = ConfigCtrl::new(dir.clone());
        // Crear un tema de usuario guardándolo.
        c.duplicate_theme("dark-blue");
        c.set_editing_name("Borrame".into());
        let id = c.save_editing_theme().unwrap();
        let id_str = id.as_str().to_string();
        assert!(c.themes.available().iter().any(|t| t.as_str() == id_str));
        // Era el activo (save lo dejó activo) → al borrar cae al default.
        assert!(c.delete_user_theme(&id_str));
        assert!(!c.themes.available().iter().any(|t| t.as_str() == id_str));
        assert_eq!(c.settings.theme, ThemeCatalog::default_id());
        // Un builtin no se borra.
        assert!(!c.delete_user_theme("dark-blue"));
        assert!(c
            .themes
            .available()
            .iter()
            .any(|t| t.as_str() == "dark-blue"));
    }

    #[test]
    fn editar_user_theme_sin_renombrar_sobrescribe_el_mismo_json() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut c = ConfigCtrl::new(dir.clone());
        // Crear un tema de usuario.
        c.duplicate_theme("dark-blue");
        c.set_editing_name("Mi Tema".into());
        let id = c.save_editing_theme().unwrap();
        let id_str = id.as_str().to_string();
        let themes_dir = dir.join("themes");
        // Editar IN-PLACE ese tema, cambiar un color, NO renombrar, guardar.
        assert_eq!(
            c.edit_user_theme(&id_str),
            id_str,
            "abre el editor con su id"
        );
        c.set_token_color(0, "#abcdef"); // accent
        let saved = c.save_editing_theme().unwrap();
        // Mismo id: NO se creó "<id>-2".
        assert_eq!(saved.as_str(), id_str, "reusa el mismo id (no duplica)");
        assert!(themes_dir.join(format!("{id_str}.json")).exists());
        assert!(
            !themes_dir.join(format!("{id_str}-2.json")).exists(),
            "no debe crear un .json huérfano con sufijo -2"
        );
        // Solo hay UN .json de usuario en el directorio.
        let user_jsons = std::fs::read_dir(&themes_dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
            .count();
        assert_eq!(user_jsons, 1, "exactamente un tema de usuario en disco");
        // El contenido se actualizó.
        let reread = c.themes.get(&ThemeId::new(&id_str));
        assert_eq!(reread.accent.to_hex(), "#abcdef");
    }

    #[test]
    fn editar_user_theme_y_renombrar_borra_el_json_viejo() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut c = ConfigCtrl::new(dir.clone());
        c.duplicate_theme("dark-blue");
        c.set_editing_name("Original".into());
        let old = c.save_editing_theme().unwrap();
        let old_str = old.as_str().to_string();
        let themes_dir = dir.join("themes");
        assert!(themes_dir.join(format!("{old_str}.json")).exists());
        // Editar y RENOMBRAR (el slug cambia).
        c.edit_user_theme(&old_str);
        c.set_editing_name("Renombrado".into());
        let new = c.save_editing_theme().unwrap();
        assert_ne!(new.as_str(), old_str, "el id nuevo refleja el nombre nuevo");
        // El .json viejo se borró; el nuevo existe.
        assert!(
            !themes_dir.join(format!("{old_str}.json")).exists(),
            "el .json del id viejo debe borrarse"
        );
        assert!(themes_dir.join(format!("{}.json", new.as_str())).exists());
        // El catálogo ya no ofrece el id viejo.
        assert!(!c.themes.available().iter().any(|t| t.as_str() == old_str));
        assert!(c
            .themes
            .available()
            .iter()
            .any(|t| t.as_str() == new.as_str()));
    }

    #[test]
    fn editar_builtin_no_abre_editor() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = ConfigCtrl::new(tmp.path().to_path_buf());
        assert_eq!(c.edit_user_theme("dark-blue"), "");
        assert!(!c.is_editing_theme());
    }

    #[test]
    fn slug_unico_evita_colision() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut c = ConfigCtrl::new(dir);
        // Guardar dos temas con el mismo nombre → ids distintos (sufijo -2).
        c.duplicate_theme("dark-blue");
        c.set_editing_name("Repe".into());
        let a = c.save_editing_theme().unwrap();
        c.duplicate_theme("dark-blue");
        c.set_editing_name("Repe".into());
        let b = c.save_editing_theme().unwrap();
        assert_ne!(a.as_str(), b.as_str());
        assert_eq!(a.as_str(), "repe");
        assert_eq!(b.as_str(), "repe-2");
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
