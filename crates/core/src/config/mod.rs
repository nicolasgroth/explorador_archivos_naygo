// Naygo — persistencia portable del workspace/plantillas/ajustes (JSON).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Carga y guarda tres archivos JSON independientes junto al ejecutable
//! (portable). Tolerante: un archivo ausente, corrupto o de versión incompatible
//! NO crashea — se cae al default y se loguea. Cada archivo es independiente.

use crate::i18n::LangId;
use crate::theme::ThemeId;
use crate::workspace::template::TemplateStore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Versión del formato de los archivos de config; permite migrar/descartar.
const CONFIG_VERSION: u32 = 1;

/// Dónde se ancla la barra de íconos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarPosition {
    Top,
    Side,
}

/// Densidad de las filas de la tabla. Compacta (default) maximiza cuántos archivos se ven;
/// Cómoda da un poco más de aire (estilo egui). La UI traduce a px.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RowDensity {
    /// 22 px por fila — máxima densidad (default; prioridad del proyecto = navegación rápida).
    #[default]
    Compact,
    /// 26 px por fila — un toque más respirado.
    Comfortable,
}

impl RowDensity {
    /// Alto de fila en px lógicos para esta densidad.
    pub fn row_height(self) -> f32 {
        match self {
            RowDensity::Compact => 22.0,
            RowDensity::Comfortable => 26.0,
        }
    }
}

/// Qué set de íconos usa la app. Flat es el default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IconSet {
    /// Multicolor plano (default).
    Flat,
    /// Estilo Fluent (Microsoft).
    Fluent,
    /// Monocromo temable (Lucide/Tabler).
    Mono,
}

/// Estilo de los íconos de la barra de herramientas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolbarIconStyle {
    /// Glifos Unicode (liviano; default).
    Glyphs,
    /// Íconos del set activo (pack).
    Pack,
}

/// Modo de ejecución de operaciones múltiples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpsMode {
    /// Una operación a la vez (las demás esperan en cola).
    Queue,
    /// Varias en paralelo.
    Parallel,
}

/// Cómo se muestra el progreso de operaciones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpsDisplay {
    /// Panel acoplado abajo (oculto si no hay ops).
    Panel,
    /// Diálogo modal.
    Modal,
    /// Panel siempre visible.
    AlwaysVisible,
}

/// Cómo se reparten los anchos de columna del file panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnWidthMode {
    /// Anchos fijos del `TableState` (resizables a mano; default histórico).
    Fixed,
    /// La tabla reparte el ancho según el contenido (egui_extras `Column::auto`).
    Auto,
}

/// Cuándo activar el modo de bajo consumo (menos repaints, sin animaciones): para
/// equipos sin GPU dedicada. `Auto` lo decide la app según el renderer detectado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LowPowerMode {
    /// Automático: ON si el render es por software (sin GPU), OFF si hay GPU.
    Auto,
    /// Siempre ON (fuerza bajo consumo aunque haya GPU).
    Always,
    /// Siempre OFF (todo activo: animaciones + framerate pleno).
    Never,
}

/// Cuánto dura el resaltado de un archivo recién aparecido (watcher).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HighlightDuration {
    /// Hasta que el usuario interactúa con el panel (default).
    UntilInteract,
    /// Se desvanece tras N segundos.
    FadeSeconds(u32),
    /// Persiste hasta re-listar la carpeta.
    UntilRefresh,
}

/// Ajustes de la app (settings.json).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub version: u32,
    pub bar_position: BarPosition,
    /// Botones de la barra solo con ícono (sin texto).
    pub icon_only: bool,
    /// Set de íconos activo. `#[serde(default)]`: un settings.json v1 previo (sin
    /// este campo) conserva el resto y solo este cae al default (honra CONFIG_VERSION).
    #[serde(default = "default_icon_set")]
    pub icon_set: String,
    /// Overrides por objeto sobre el set base: clave estable (`"action_back"`,
    /// `"file_image"`, …) → fuente del ícono. Vacío = solo el set base.
    #[serde(default)]
    pub icon_overrides: std::collections::BTreeMap<String, crate::icon_source::IconSource>,
    /// Estilo de los íconos de la barra de herramientas (glifos vs pack). `#[serde(default)]`
    /// por retro-compat.
    #[serde(default = "default_toolbar_icon_style")]
    pub toolbar_icon_style: ToolbarIconStyle,
    /// Color de los glifos de la barra (cuando el estilo es Glyphs). `None` = usa el color
    /// del tema. `#[serde(default)]` por retro-compat.
    #[serde(default)]
    pub toolbar_glyph_color: Option<crate::theme::ThemeColor>,
    /// Mostrar la fila virtual ".." al tope del panel de archivos. `#[serde(default)]`
    /// por la misma razón que `icon_set`.
    #[serde(default = "default_show_parent_entry")]
    pub show_parent_entry: bool,
    /// Idioma activo de la UI. Vacío/ausente → se detecta del SO al arrancar.
    #[serde(default = "default_language")]
    pub language: LangId,
    /// Tema (color set) activo. `#[serde(default)]` por retro-compat (settings viejo
    /// sin este campo cae al default).
    #[serde(default = "default_theme")]
    pub theme: ThemeId,
    /// Modo de ejecución de operaciones múltiples. `#[serde(default)]` por retro-compat.
    #[serde(default = "default_ops_mode")]
    pub ops_mode: OpsMode,
    /// Cómo se muestra el progreso de operaciones. `#[serde(default)]` por retro-compat.
    #[serde(default = "default_ops_display")]
    pub ops_display: OpsDisplay,
    /// Confirmar también el borrado a papelera (el permanente siempre confirma).
    #[serde(default)]
    pub confirm_trash: bool,
    /// Mostrar el resumen al terminar una operación. `#[serde(default)]` por retro-compat.
    #[serde(default = "default_show_op_summary")]
    pub show_op_summary: bool,
    /// Pegar texto/imagen: pedir confirmación de nombre antes de crear (modo B).
    /// `false` (default) = crear directo con nombre automático (modo A).
    #[serde(default = "default_paste_confirm")]
    pub paste_confirm: bool,
    /// Plantilla de nombre para un archivo de texto pegado. `{fecha}` → fecha/hora.
    #[serde(default = "default_paste_text_name")]
    pub paste_text_name: String,
    /// Extensión (sin punto) para texto pegado.
    #[serde(default = "default_paste_text_ext")]
    pub paste_text_ext: String,
    /// Plantilla de nombre para una imagen pegada. `{fecha}` → fecha/hora.
    #[serde(default = "default_paste_image_name")]
    pub paste_image_name: String,
    /// Formato de salida para imagen pegada.
    #[serde(default = "default_paste_image_fmt")]
    pub paste_image_fmt: crate::clipboard::ImageFmt,
    /// Calidad JPG (1..=100) para imagen pegada como JPG.
    #[serde(default = "default_paste_jpg_quality")]
    pub paste_jpg_quality: u8,
    /// Duración del resaltado de archivos recién aparecidos (watcher).
    #[serde(default = "default_highlight_duration")]
    pub highlight_duration: HighlightDuration,
    /// Si los archivos nuevos se agrupan al final del listado (resaltados) en vez de
    /// insertarse ya ordenados.
    #[serde(default = "default_new_items_at_end")]
    pub new_items_at_end: bool,
    /// Al calcular el tamaño de una carpeta (F3), NO bajar a subdirectorios (solo el
    /// primer nivel). Más barato. `false` (default) = recursivo.
    #[serde(default = "default_size_no_subdirs")]
    pub size_no_subdirs: bool,
    /// Ícono de Naygo en la bandeja del sistema (junto al reloj). `true` por defecto:
    /// pedido de Nicolás (acceso rápido). `#[serde(default)]` retro-compat.
    #[serde(default = "default_tray_enabled")]
    pub tray_enabled: bool,
    /// Al cerrar la ventana, ocultar a la bandeja en vez de salir. Opt-in (default
    /// `false`): residente = memoria ocupada, que sea decisión del usuario.
    #[serde(default)]
    pub close_to_tray: bool,
    /// Iniciar Naygo con Windows (entrada Run del registro). `#[serde(default)]` retro-compat.
    #[serde(default)]
    pub autostart: bool,
    /// Formato de las columnas de fecha (Modificado/Creado). `#[serde(default)]` retro-compat.
    #[serde(default)]
    pub date_format: crate::format::DateFormat,
    /// Formato de la columna de tamaño (Auto/Bytes/KB/MB). `#[serde(default)]` retro-compat.
    #[serde(default)]
    pub size_format: crate::format::SizeFormat,
    /// Densidad de las filas (Compacta/Cómoda). `#[serde(default)]` retro-compat.
    #[serde(default)]
    pub row_density: RowDensity,
    /// Caché de carpetas visitadas: nº máximo de carpetas recordadas en memoria
    /// (0 = desactivado). Volver a una carpeta cacheada pinta al instante y el
    /// listado real corre por detrás (stale-while-revalidate).
    #[serde(default = "default_cache_max_dirs")]
    pub cache_max_dirs: usize,
    /// Modo de ancho de columnas de los file panels: fijo (resizable a mano) o
    /// automático (la tabla reparte por contenido). `#[serde(default)]` retro-compat.
    #[serde(default = "default_column_width_mode")]
    pub column_width_mode: ColumnWidthMode,
    /// Plantilla de tabla para los paneles NUEVOS: columnas visibles, orden y anchos
    /// tomados del panel activo con «Usar el panel activo como predeterminado». `None`
    /// (default) = los paneles nuevos nacen con `TableState::default()`.
    #[serde(default)]
    pub default_table: Option<crate::columns::TableState>,
    /// Reglas de previsualización (una por extensión): toggle + alias. Editable en
    /// Configuración → Previsualización. `#[serde(default)]` retro-compat.
    #[serde(default = "default_preview_rules_cfg")]
    pub preview_rules: Vec<crate::preview::PreviewRule>,
    /// DEPRECADO: CSV de extensiones de texto (lote 2). Solo se LEE para migrar a
    /// `preview_rules`. Ya no se escribe (skip si está vacío). El `rename` lo hace leer
    /// el campo `preview_text_exts` de un settings.json del lote 2.
    #[serde(
        default,
        rename = "preview_text_exts",
        skip_serializing_if = "String::is_empty"
    )]
    pub preview_text_exts_legacy: String,
    /// Cuándo usar el modo de bajo consumo. `#[serde(default)]` retro-compat (settings
    /// previos → Auto).
    #[serde(default = "default_low_power_mode")]
    pub low_power_mode: LowPowerMode,
    /// Cuántas carpetas recientes recordar (1–100). `#[serde(default)]` por retro-compat
    /// (settings viejo sin el campo → 50). El uso real lo clampa a 1..=100.
    #[serde(default = "default_recent_limit")]
    pub recent_limit: usize,
    /// Mostrar el footer (barra inferior) en cada panel de archivos. `#[serde(default)]`
    /// retro-compat (settings viejo → true).
    #[serde(default = "default_footer_enabled")]
    pub footer_enabled: bool,
    /// Plantilla del footer (global a todos los paneles). `#[serde(default)]` retro-compat.
    #[serde(default)]
    pub footer_preset: crate::footer::FooterPreset,
    /// Template personalizado del footer (cuando `footer_preset == Custom`). `#[serde(default)]`.
    #[serde(default)]
    pub footer_custom_template: String,
    /// Resaltar automáticamente el código de extensiones conocidas en modo Auto del preview.
    /// `#[serde(default)]` retro-compat (settings viejo → true).
    #[serde(default = "default_auto_highlight_code")]
    pub auto_highlight_code: bool,
    /// Carpeta de inicio (botón Home). Vacío = carpeta personal del usuario. `#[serde(default)]`.
    #[serde(default)]
    pub home_dir: String,
    /// Mostrar archivos/carpetas con atributo oculto (HIDDEN). Default true (Naygo muestra todo).
    #[serde(default = "default_show_hidden")]
    pub show_hidden: bool,
    /// Mostrar archivos/carpetas con atributo de sistema (SYSTEM). Default true.
    #[serde(default = "default_show_system")]
    pub show_system: bool,
    /// Ocultar los que empiezan con punto (dotfiles estilo Linux). Default false.
    #[serde(default)]
    pub hide_dotfiles: bool,
    /// Preguntar (modal de confirmación) al arrastrar archivos/carpetas entre paneles antes de
    /// copiar o mover. Default `true`: red de seguridad contra arrastres accidentales. En `false`
    /// el drop se ejecuta directo. OJO: el modal de CONFLICTO (archivo que ya existe en el destino)
    /// es independiente y SIEMPRE aparece, esté esto en `true` o `false`.
    #[serde(default = "default_confirm_drop_between_panes")]
    pub confirm_drop_between_panes: bool,
}

/// Resuelve la carpeta Home: si `home_dir` está vacío, usa la carpeta personal del usuario
/// (`%USERPROFILE%`); si tiene una ruta, usa esa. Pura, testeable.
pub fn resolve_home_dir(home_dir: &str) -> PathBuf {
    if !home_dir.trim().is_empty() {
        return PathBuf::from(home_dir);
    }
    // Carpeta personal del usuario vía USERPROFILE; último recurso: la raíz C:\
    // (nunca vacío, nunca panic). `naygo-core` NO tiene la crate `dirs`.
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\"))
}

/// Default de `low_power_mode`: Auto (lo decide la app según el renderer).
fn default_low_power_mode() -> LowPowerMode {
    LowPowerMode::Auto
}

/// Default de `recent_limit`: 50 carpetas recientes.
fn default_recent_limit() -> usize {
    50
}

/// Default de `footer_enabled`: true (mostrar el footer).
fn default_footer_enabled() -> bool {
    true
}

/// Default de `auto_highlight_code`: true (resaltar código en Auto).
fn default_auto_highlight_code() -> bool {
    true
}

/// Default de `preview_rules`: las reglas semilla (texto + imagen, habilitadas).
fn default_preview_rules_cfg() -> Vec<crate::preview::PreviewRule> {
    crate::preview::default_preview_rules()
}

/// Default de `column_width_mode` para `#[serde(default)]`: fijo (comportamiento previo).
fn default_column_width_mode() -> ColumnWidthMode {
    ColumnWidthMode::Fixed
}

/// Default de `cache_max_dirs` para `#[serde(default)]`.
fn default_cache_max_dirs() -> usize {
    50
}

/// Default de `tray_enabled` para `#[serde(default)]`.
fn default_tray_enabled() -> bool {
    true
}

/// Default de `icon_set` para `#[serde(default)]` (campo aditivo retro-compatible).
fn default_icon_set() -> String {
    "lucide".to_string()
}

/// Default de `toolbar_icon_style` para `#[serde(default)]`: glifos Unicode.
fn default_toolbar_icon_style() -> ToolbarIconStyle {
    ToolbarIconStyle::Glyphs
}

/// Default de `show_parent_entry` para `#[serde(default)]`.
fn default_show_parent_entry() -> bool {
    true
}

/// Default de `language` para `#[serde(default)]`. La detección real del SO la hace
/// la capa ui en el primer arranque; aquí un marcador neutro ("en").
fn default_language() -> LangId {
    LangId::new("en")
}

/// Default de `theme` para `#[serde(default)]`: Dark Blue.
fn default_theme() -> ThemeId {
    ThemeId::new("dark-blue")
}

/// Default de `ops_mode` para `#[serde(default)]`: cola (una a la vez).
fn default_ops_mode() -> OpsMode {
    OpsMode::Queue
}

/// Default de `ops_display` para `#[serde(default)]`: panel acoplado.
fn default_ops_display() -> OpsDisplay {
    OpsDisplay::Panel
}

/// Default de `show_op_summary` para `#[serde(default)]`.
fn default_show_op_summary() -> bool {
    true
}

/// Default de `paste_confirm`: false (crear directo).
fn default_paste_confirm() -> bool {
    false
}

/// Default de `paste_text_name`.
fn default_paste_text_name() -> String {
    "pegado {fecha}".to_string()
}

/// Default de `paste_text_ext`.
fn default_paste_text_ext() -> String {
    "txt".to_string()
}

/// Default de `paste_image_name`.
fn default_paste_image_name() -> String {
    "captura {fecha}".to_string()
}

/// Default de `paste_image_fmt`: PNG.
fn default_paste_image_fmt() -> crate::clipboard::ImageFmt {
    crate::clipboard::ImageFmt::Png
}

/// Default de `paste_jpg_quality`: 90.
fn default_paste_jpg_quality() -> u8 {
    90
}

/// Default de `highlight_duration`: UntilInteract.
fn default_highlight_duration() -> HighlightDuration {
    HighlightDuration::UntilInteract
}

/// Default de `new_items_at_end`: false.
fn default_new_items_at_end() -> bool {
    false
}

/// Default de `size_no_subdirs`: false (recursivo).
fn default_size_no_subdirs() -> bool {
    false
}

/// Default de `show_hidden`: true (Naygo muestra los ocultos por defecto).
fn default_show_hidden() -> bool {
    true
}

/// Default de `show_system`: true.
fn default_show_system() -> bool {
    true
}

/// Default de `confirm_drop_between_panes`: true (preguntar antes de copiar/mover entre paneles).
fn default_confirm_drop_between_panes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: CONFIG_VERSION,
            bar_position: BarPosition::Top,
            icon_only: true,
            icon_set: "lucide".into(),
            icon_overrides: std::collections::BTreeMap::new(),
            toolbar_icon_style: ToolbarIconStyle::Glyphs,
            toolbar_glyph_color: None,
            show_parent_entry: true,
            language: default_language(),
            theme: default_theme(),
            ops_mode: OpsMode::Queue,
            ops_display: OpsDisplay::Panel,
            confirm_trash: false,
            show_op_summary: true,
            paste_confirm: false,
            paste_text_name: "pegado {fecha}".into(),
            paste_text_ext: "txt".into(),
            paste_image_name: "captura {fecha}".into(),
            paste_image_fmt: crate::clipboard::ImageFmt::Png,
            paste_jpg_quality: 90,
            highlight_duration: HighlightDuration::UntilInteract,
            new_items_at_end: false,
            size_no_subdirs: false,
            tray_enabled: true,
            close_to_tray: false,
            autostart: false,
            date_format: crate::format::DateFormat::IsoMinute,
            size_format: crate::format::SizeFormat::Auto,
            row_density: RowDensity::Compact,
            cache_max_dirs: 50,
            column_width_mode: ColumnWidthMode::Fixed,
            default_table: None,
            preview_rules: default_preview_rules_cfg(),
            preview_text_exts_legacy: String::new(),
            low_power_mode: LowPowerMode::Auto,
            recent_limit: 50,
            footer_enabled: true,
            footer_preset: crate::footer::FooterPreset::Compact,
            footer_custom_template: String::new(),
            auto_highlight_code: true,
            home_dir: String::new(),
            show_hidden: true,
            show_system: true,
            hide_dotfiles: false,
            confirm_drop_between_panes: true,
        }
    }
}

/// Estado persistible del workspace (workspace.json).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspacePersist {
    pub version: u32,
    /// La disposición (árbol de splits con PaneId).
    pub layout: crate::workspace::layout::SerializableDockLayout,
    /// El id del panel activo.
    pub active: Option<crate::workspace::PaneId>,
    /// Estado persistible de cada panel Files, indexado por PaneId.
    pub files: Vec<(
        crate::workspace::PaneId,
        crate::workspace::file_pane::FilePanePersist,
    )>,
    /// Tipo de cada panel del layout (para reconstruir Tree/Inspector también).
    pub purposes: Vec<(crate::workspace::PaneId, crate::workspace::PanePurpose)>,
}

/// Lee un archivo JSON y lo deserializa, devolviendo `None` si no existe o falla.
fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    read_json_recovering(path).0
}

/// Como `read_json`, pero con ARRANQUE SEGURO: si el archivo existe y está corrupto
/// (no parsea), lo renombra a `<nombre>.json.bad` (backup: no se pierde y no se
/// reintenta al próximo arranque) y devuelve `(None, true)` — el llamador cae a
/// defaults y puede avisar al usuario. `(_, false)` = no había archivo o se leyó bien.
fn read_json_recovering<T: for<'de> Deserialize<'de>>(path: &Path) -> (Option<T>, bool) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (None, false);
    };
    match serde_json::from_str::<T>(&text) {
        Ok(v) => (Some(v), false),
        Err(e) => {
            tracing::warn!(
                "config ilegible en {}: {e}; respaldando como .bad",
                path.display()
            );
            let bad = path.with_extension("json.bad");
            if let Err(re) = std::fs::rename(path, &bad) {
                tracing::warn!("no se pudo respaldar {}: {re}", path.display());
            }
            (None, true)
        }
    }
}

/// Escribe un valor como JSON (pretty). Loguea y traga el error (nunca crashea).
fn write_json<T: Serialize>(path: &Path, value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => {
            if let Err(e) = std::fs::write(path, text) {
                tracing::warn!("no se pudo guardar {}: {e}", path.display());
            }
        }
        Err(e) => tracing::warn!("no se pudo serializar {}: {e}", path.display()),
    }
}

/// Normaliza el id del set de íconos: mapea las formas capitalizadas del enum viejo
/// (`"Flat"`, `"Fluent"`, `"Mono"`) a los ids en minúscula actuales. Cualquier otro id
/// se conserva tal cual (packs sueltos; el catálogo/IconProvider resuelven los desconocidos).
fn normalize_icon_set_id(id: &str) -> String {
    match id {
        "Flat" | "flat" => "flat-color".to_string(),
        "Fluent" | "fluent" => "lucide".to_string(),
        "Mono" => "mono".to_string(),
        other => other.to_string(),
    }
}

/// Carga settings; si falta/corrupto/versión incompatible → default.
pub fn load_settings(dir: &Path) -> Settings {
    load_settings_flagged(dir).0
}

/// Como `load_settings`, pero informa si hubo RECUPERACIÓN (archivo corrupto
/// respaldado como .bad y defaults aplicados) para que la UI avise al usuario.
pub fn load_settings_flagged(dir: &Path) -> (Settings, bool) {
    let (read, recovered) = read_json_recovering::<Settings>(&dir.join("settings.json"));
    let settings = match read {
        Some(mut s) if s.version == CONFIG_VERSION => {
            // Migra el formato viejo y luego coacciona contra el catálogo: un id de pack
            // suelto que ya no existe en disco (carpeta borrada) cae a "flat" en vez de
            // quedar como selección colgada con la toolbar rota.
            let normalized = normalize_icon_set_id(&s.icon_set);
            s.icon_set = crate::icon_set::IconSetCatalog::load(dir).resolve(&normalized);
            // Migrar el CSV de preview (lote 2) a reglas, si venía el campo viejo y no
            // hay reglas explícitas (settings anterior al lote 3). Si tras eso siguen
            // vacías (settings raro), caer a las reglas por defecto.
            if !s.preview_text_exts_legacy.is_empty() && s.preview_rules.is_empty() {
                s.preview_rules = crate::preview::rules_from_csv(&s.preview_text_exts_legacy);
            }
            if s.preview_rules.is_empty() {
                s.preview_rules = crate::preview::default_preview_rules();
            }
            s.preview_text_exts_legacy.clear();
            s
        }
        Some(_) => {
            tracing::warn!("settings.json de versión incompatible; usando default");
            Settings::default()
        }
        None => Settings::default(),
    };
    (settings, recovered)
}

/// Guarda settings.
pub fn save_settings(dir: &Path, s: &Settings) {
    write_json(&dir.join("settings.json"), s);
}

/// Carga el store de plantillas; si falta/corrupto → vacío.
pub fn load_templates(dir: &Path) -> TemplateStore {
    read_json::<TemplateStore>(&dir.join("templates.json")).unwrap_or_default()
}

/// Guarda el store de plantillas.
pub fn save_templates(dir: &Path, store: &TemplateStore) {
    write_json(&dir.join("templates.json"), store);
}

/// Carga el workspace persistido; `None` si falta/corrupto/versión incompatible
/// (el llamador cae a la plantilla default).
pub fn load_workspace(dir: &Path) -> Option<WorkspacePersist> {
    load_workspace_flagged(dir).0
}

/// Como `load_workspace`, pero informa si hubo RECUPERACIÓN (archivo corrupto
/// respaldado como .bad) para que la UI avise al usuario.
pub fn load_workspace_flagged(dir: &Path) -> (Option<WorkspacePersist>, bool) {
    let (read, recovered) = read_json_recovering::<WorkspacePersist>(&dir.join("workspace.json"));
    let ws = match read {
        Some(w) if w.version == CONFIG_VERSION => Some(w),
        Some(_) => {
            tracing::warn!("workspace.json de versión incompatible; ignorando");
            None
        }
        None => None,
    };
    (ws, recovered)
}

/// Guarda el workspace persistido.
pub fn save_workspace(dir: &Path, w: &WorkspacePersist) {
    write_json(&dir.join("workspace.json"), w);
}

/// Carga el keymap desde `keybindings.json`; ausente/corrupto → defaults.
pub fn load_keymap(dir: &Path) -> crate::keymap::KeyMap {
    read_json::<crate::keymap::KeyMap>(&dir.join("keybindings.json"))
        .unwrap_or_else(crate::keymap::KeyMap::defaults)
}

/// Guarda el keymap.
pub fn save_keymap(dir: &Path, km: &crate::keymap::KeyMap) {
    write_json(&dir.join("keybindings.json"), km);
}

/// Directorio de config portable: junto al ejecutable, o el cwd como fallback.
pub fn portable_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_corrupto_se_respalda_y_cae_a_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ esto no es json válido ]]").unwrap();
        let (s, recovered) = load_settings_flagged(dir.path());
        assert!(recovered, "debe reportar recuperación");
        assert_eq!(s, Settings::default());
        assert!(!path.exists(), "el corrupto no debe quedar en su lugar");
        assert!(
            dir.path().join("settings.json.bad").exists(),
            "debe quedar el respaldo .bad"
        );
    }

    #[test]
    fn settings_ausente_no_es_recuperacion() {
        let dir = tempfile::tempdir().unwrap();
        let (s, recovered) = load_settings_flagged(dir.path());
        assert!(!recovered);
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn workspace_corrupto_se_respalda() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        std::fs::write(&path, "no-json").unwrap();
        let (w, recovered) = load_workspace_flagged(dir.path());
        assert!(recovered);
        assert!(w.is_none());
        assert!(dir.path().join("workspace.json.bad").exists());
    }

    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let s = Settings {
            version: CONFIG_VERSION,
            bar_position: BarPosition::Side,
            icon_only: false,
            icon_set: "mono".to_string(),
            icon_overrides: std::collections::BTreeMap::new(),
            toolbar_icon_style: ToolbarIconStyle::Pack,
            toolbar_glyph_color: Some(crate::theme::ThemeColor::new(0x10, 0x20, 0x30)),
            show_parent_entry: false,
            language: default_language(),
            theme: default_theme(),
            ops_mode: OpsMode::Parallel,
            ops_display: OpsDisplay::Modal,
            confirm_trash: true,
            show_op_summary: false,
            paste_confirm: true,
            paste_text_name: "nota {fecha}".into(),
            paste_text_ext: "md".into(),
            paste_image_name: "img {fecha}".into(),
            paste_image_fmt: crate::clipboard::ImageFmt::Jpg,
            paste_jpg_quality: 75,
            highlight_duration: HighlightDuration::FadeSeconds(6),
            new_items_at_end: true,
            size_no_subdirs: true,
            tray_enabled: false,
            close_to_tray: true,
            autostart: false,
            date_format: crate::format::DateFormat::DmyMinute,
            size_format: crate::format::SizeFormat::Kb,
            row_density: RowDensity::Comfortable,
            cache_max_dirs: 12,
            column_width_mode: ColumnWidthMode::Auto,
            default_table: Some({
                let mut t = crate::columns::TableState::default();
                t.toggle_visible(crate::columns::ColumnKind::Created);
                t.set_width(crate::columns::ColumnKind::Name, 333.0);
                t
            }),
            preview_rules: vec![
                crate::preview::PreviewRule {
                    ext: "sif".into(),
                    enabled: true,
                    view: crate::preview::ViewMode::Code(crate::preview::CodeLang::Xml),
                },
                crate::preview::PreviewRule {
                    ext: "png".into(),
                    enabled: false,
                    view: crate::preview::ViewMode::Auto,
                },
            ],
            preview_text_exts_legacy: String::new(),
            low_power_mode: LowPowerMode::Always,
            recent_limit: 25,
            footer_enabled: false,
            footer_preset: crate::footer::FooterPreset::Full,
            footer_custom_template: "{sel}/{total}".into(),
            auto_highlight_code: false,
            home_dir: "D:\\Trabajo".into(),
            show_hidden: false,
            show_system: false,
            hide_dotfiles: true,
            confirm_drop_between_panes: false,
        };
        save_settings(dir.path(), &s);
        assert_eq!(load_settings(dir.path()), s);
    }

    #[test]
    fn settings_viejo_sin_low_power_cae_a_auto() {
        let json = r#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"flat"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.low_power_mode, LowPowerMode::Auto);
    }

    #[test]
    fn settings_migra_preview_csv_a_reglas() {
        let dir = tempfile::tempdir().unwrap();
        // settings.json del lote 2: trae preview_text_exts (CSV), sin preview_rules.
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"flat","preview_text_exts":"txt, md"}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert!(s.preview_rules.iter().any(|r| r.ext == "txt" && r.enabled));
        assert!(s.preview_rules.iter().any(|r| r.ext == "md"));
        // Las imágenes se agregan en la migración.
        assert!(s.preview_rules.iter().any(|r| r.ext == "png"));
        assert!(
            s.preview_text_exts_legacy.is_empty(),
            "el CSV viejo se limpia"
        );
    }

    #[test]
    fn settings_viejo_sin_columnas_cae_a_fijo_y_sin_plantilla() {
        // Un settings.json previo sin los campos de columnas debe conservar el resto y
        // que los nuevos caigan a su default (Fixed, None) por #[serde(default)].
        let json = r#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"flat","show_parent_entry":true,"language":"es","theme":"dark-blue"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.column_width_mode, ColumnWidthMode::Fixed);
        assert!(s.default_table.is_none());
    }

    #[test]
    fn settings_round_trip_con_idioma() {
        let dir = tempfile::tempdir().unwrap();
        let s = Settings {
            language: crate::i18n::LangId::new("es"),
            ..Settings::default()
        };
        save_settings(dir.path(), &s);
        assert_eq!(
            load_settings(dir.path()).language,
            crate::i18n::LangId::new("es")
        );
    }

    #[test]
    fn settings_v1_sin_idioma_cae_a_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"Flat","show_parent_entry":true}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert_eq!(s.language, crate::i18n::LangId::new("en"));
    }

    #[test]
    fn settings_default_tiene_iconos_flat_y_fila_padre_on() {
        let s = Settings::default();
        assert_eq!(s.icon_set, "lucide");
        assert!(s.show_parent_entry);
    }

    #[test]
    fn toolbar_defaults_y_round_trip() {
        let mut s = Settings::default();
        assert_eq!(s.toolbar_icon_style, ToolbarIconStyle::Glyphs);
        assert!(s.toolbar_glyph_color.is_none());
        s.toolbar_icon_style = ToolbarIconStyle::Pack;
        s.toolbar_glyph_color = Some(crate::theme::ThemeColor::new(0xe0, 0xa0, 0x30));
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.toolbar_icon_style, ToolbarIconStyle::Pack);
        assert_eq!(back.toolbar_glyph_color, s.toolbar_glyph_color);
    }

    #[test]
    fn icon_set_id_default_es_flat() {
        // El default cambió a "lucide" (nuevo set de fábrica principal).
        assert_eq!(Settings::default().icon_set, "lucide");
    }

    #[test]
    fn icon_set_migra_del_enum_viejo() {
        // "Flat" (mayúscula, enum viejo) → normaliza a "flat-color".
        // "Mono" sigue siendo "mono" (no cambió).
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("icons").join("flat-color")).unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"bar_position":"Top","icon_only":false,"icon_set":"Flat"}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert_eq!(s.icon_set, "flat-color");

        // "Mono" (mayúscula) sigue resolviendo a "mono" (embebido).
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"bar_position":"Top","icon_only":false,"icon_set":"Mono"}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert_eq!(s.icon_set, "mono");
    }

    #[test]
    fn icon_set_de_pack_borrado_cae_a_flat() {
        // Un id de pack suelto guardado cuya carpeta ya no existe en disco se coacciona
        // a "flat" al cargar (no queda como selección colgada con la toolbar rota).
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"bar_position":"Top","icon_only":false,"icon_set":"pack-borrado"}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert_eq!(s.icon_set, "flat");
    }

    #[test]
    fn icon_set_de_pack_suelto_existente_se_conserva() {
        // Si la carpeta del pack suelto sí existe en <config>/icons/<id>/, el id se conserva.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("icons").join("mi-pack")).unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"bar_position":"Top","icon_only":false,"icon_set":"mi-pack"}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert_eq!(s.icon_set, "mi-pack");
    }

    #[test]
    fn settings_ausente_da_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
    }

    #[test]
    fn settings_v1_sin_campos_nuevos_conserva_los_viejos() {
        // Un settings.json v1 escrito por un build previo (sin icon_set ni
        // show_parent_entry) debe conservar bar_position/icon_only y solo caer al
        // default en los campos nuevos, gracias a #[serde(default)] (honra CONFIG_VERSION).
        // Nota: el default de icon_set es "lucide", pero el catálogo actual (Task 2)
        // todavía no tiene "lucide" como embebido → resolve lo cae a "flat". En Task 3
        // se agrega "lucide" como embebido y este test se actualizará.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"bar_position":"Side","icon_only":false}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert_eq!(s.bar_position, BarPosition::Side, "conserva lo viejo");
        assert!(!s.icon_only, "conserva lo viejo");
        assert_eq!(s.icon_set, "flat", "lucide aún no es embebido → resolve cae a flat");
        assert!(s.show_parent_entry, "campo nuevo cae al default");
    }

    #[test]
    fn settings_default_theme_es_dark_blue() {
        use crate::theme::ThemeId;
        let s = Settings::default();
        assert_eq!(s.theme, ThemeId::new("dark-blue"));
    }

    #[test]
    fn settings_viejo_sin_theme_cae_al_default() {
        use crate::theme::ThemeId;
        let json = r#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"Flat","show_parent_entry":true,"language":"es"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.theme, ThemeId::new("dark-blue"));
    }

    #[test]
    fn settings_default_ops() {
        let s = Settings::default();
        assert_eq!(s.ops_mode, OpsMode::Queue);
        assert_eq!(s.ops_display, OpsDisplay::Panel);
        assert!(!s.confirm_trash);
        assert!(s.show_op_summary);
    }

    #[test]
    fn settings_viejo_sin_ops_cae_a_defaults() {
        let json = r#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"Flat","show_parent_entry":true,"language":"es","theme":"dark-blue"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.ops_mode, OpsMode::Queue);
        assert!(s.show_op_summary);
    }

    #[test]
    fn settings_corrupto_da_default_sin_panic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), b"{ no es json valido").unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
    }

    #[test]
    fn settings_version_incompatible_da_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":999,"bar_position":"Top","icon_only":true}"#,
        )
        .unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
    }

    #[test]
    fn templates_ausente_da_vacio() {
        let dir = tempfile::tempdir().unwrap();
        let store = load_templates(dir.path());
        assert!(store.user.is_empty());
        assert!(store.recents.is_empty());
    }

    #[test]
    fn workspace_ausente_da_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_workspace(dir.path()).is_none());
    }

    #[test]
    fn workspace_round_trip_conserva_layout_y_activo() {
        use crate::workspace::layout::SerializableDockLayout;
        use crate::workspace::PaneId;

        let dir = tempfile::tempdir().unwrap();
        let persist = WorkspacePersist {
            version: CONFIG_VERSION,
            layout: SerializableDockLayout::single(PaneId(3)),
            active: Some(PaneId(3)),
            files: Vec::new(),
            purposes: vec![(PaneId(3), crate::workspace::PanePurpose::Files)],
        };
        save_workspace(dir.path(), &persist);
        let loaded = load_workspace(dir.path()).expect("debe cargar");
        assert_eq!(loaded.version, CONFIG_VERSION);
        assert_eq!(loaded.active, Some(PaneId(3)));
        assert_eq!(loaded.layout.pane_ids(), vec![PaneId(3)]);
        assert_eq!(loaded.purposes.len(), 1);
    }

    #[test]
    fn keymap_ausente_es_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let km = load_keymap(dir.path());
        assert_eq!(km, crate::keymap::KeyMap::defaults());
    }

    #[test]
    fn keymap_round_trip_en_disco() {
        use crate::keymap::{Action, Chord, KeyCode};
        let dir = tempfile::tempdir().unwrap();
        let mut km = crate::keymap::KeyMap::defaults();
        km.bind(Action::Copy, Chord::ctrl(KeyCode::Char('z')));
        save_keymap(dir.path(), &km);
        let back = load_keymap(dir.path());
        assert_eq!(
            back.action_for(&Chord::ctrl(KeyCode::Char('z'))),
            Some(Action::Copy)
        );
    }

    #[test]
    fn keymap_corrupto_es_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keybindings.json"), b"{ no es json").unwrap();
        let km = load_keymap(dir.path());
        assert_eq!(km, crate::keymap::KeyMap::defaults());
    }

    #[test]
    fn recent_limit_default_es_50() {
        // (a) El default del struct es 50.
        let s = Settings::default();
        assert_eq!(s.recent_limit, 50);
    }

    #[test]
    fn recent_limit_sin_campo_deserializa_a_50() {
        // (b) Un JSON sin el campo (settings viejo) cae al default 50 vía #[serde(default)].
        let json = r#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"flat"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.recent_limit, 50);
    }

    #[test]
    fn settings_nuevos_tienen_defaults() {
        let s = Settings::default();
        assert!(s.footer_enabled);
        assert_eq!(s.footer_preset, crate::footer::FooterPreset::Compact);
        assert!(s.footer_custom_template.is_empty());
        assert!(s.auto_highlight_code);
        assert!(s.home_dir.is_empty());
    }

    #[test]
    fn settings_viejo_sin_campos_nuevos_migra_a_defaults() {
        // Un JSON v1 mínimo SIN los campos nuevos debe cargar con defaults (serde default).
        let json = r#"{"version":1,"bar_position":"Top","icon_only":false}"#;
        let s: Settings = serde_json::from_str(json).expect("debe migrar");
        assert!(s.footer_enabled);
        assert!(s.auto_highlight_code);
        assert!(s.home_dir.is_empty());
    }

    #[test]
    fn settings_visibilidad_defaults() {
        let s = Settings::default();
        assert!(s.show_hidden); // mostrar todo por defecto
        assert!(s.show_system);
        assert!(!s.hide_dotfiles);
    }

    #[test]
    fn settings_viejo_migra_visibilidad() {
        // Un settings.json sin estos campos: show_hidden/show_system caen a true (default fn).
        let json = r#"{"version":1,"bar_position":"Top","icon_only":false}"#;
        let s: Settings = serde_json::from_str(json).expect("debe migrar");
        assert!(s.show_hidden);
        assert!(s.show_system);
        assert!(!s.hide_dotfiles);
        // Un settings.json viejo sin el campo conserva la red de seguridad (preguntar = true).
        assert!(s.confirm_drop_between_panes);
    }

    #[test]
    fn settings_confirm_drop_default_es_true() {
        let s = Settings::default();
        assert!(
            s.confirm_drop_between_panes,
            "por defecto se pregunta antes de copiar/mover entre paneles"
        );
    }

    #[test]
    fn home_vacio_cae_al_perfil_del_usuario() {
        // Una ruta explícita se respeta tal cual.
        let explicit = resolve_home_dir("D:\\Trabajo");
        assert_eq!(explicit, std::path::PathBuf::from("D:\\Trabajo"));

        // Vacío → alguna ruta no vacía (el perfil del usuario; varía por máquina).
        let fallback = resolve_home_dir("");
        assert!(
            !fallback.as_os_str().is_empty(),
            "vacío debe resolver a una ruta"
        );
    }

    #[test]
    fn icon_set_default_es_lucide() {
        assert_eq!(Settings::default().icon_set, "lucide");
        assert!(Settings::default().icon_overrides.is_empty());
    }

    #[test]
    fn migra_flat_a_flat_color_y_fluent_a_lucide() {
        let dir = tempfile::tempdir().unwrap();
        for s in ["lucide", "flat-color"] {
            std::fs::create_dir_all(dir.path().join("icons").join(s)).unwrap();
        }
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"bar_position":"Top","icon_only":false,"icon_set":"flat"}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert_eq!(s.icon_set, "flat-color");

        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"bar_position":"Top","icon_only":false,"icon_set":"fluent"}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert_eq!(s.icon_set, "lucide");
    }

    #[test]
    fn overrides_persisten_round_trip() {
        use crate::icon_source::IconSource;
        let dir = tempfile::tempdir().unwrap();
        let mut s = Settings::default();
        s.icon_overrides
            .insert("folder".into(), IconSource::Builtin { set_id: "material".into() });
        save_settings(dir.path(), &s);
        let back = load_settings(dir.path());
        assert_eq!(back.icon_overrides.get("folder"),
            Some(&IconSource::Builtin { set_id: "material".into() }));
    }
}
