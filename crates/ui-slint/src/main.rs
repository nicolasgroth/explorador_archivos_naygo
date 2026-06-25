// Naygo — arranque de la capa UI en Slint (Fase 2b: multi-panel + paneles especiales).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Subsistema GUI en release: sin ventana de consola negra al lanzar el .exe. En debug se
// conserva la consola para ver stderr/logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//
// Para forzar el renderizador por software (caso VM sin GPU):
//   $env:SLINT_BACKEND="winit-software"; cargo run -p naygo-ui-slint
//
// MODELOS ESTABLES (clave del rendimiento y de la corrección):
// Slint es modo retenido: un `for p in root.panes` recrea un panel por cada ELEMENTO del
// modelo. Si se reemplaza el VecModel entero en cada refresco, Slint destruye y recrea cada
// panel + sus ListView en cada tick → se pierde el scroll y se cortan los gestos. Por eso
// mantenemos modelos ESTABLES y los mutamos in situ:
//   - `panes`: un VecModel<PaneVm> que solo se reestructura cuando cambia la LISTA de
//     paneles o el ÁREA (agregar/quitar panel, resize).
//   - Por panel, según su tipo, un VecModel ESTABLE de filas (Files/Tree/Favoritos/
//     Recientes/Historial) que se actualiza con `set_vec` (mismo VecModel) → los ListView
//     conservan su scroll. Inspector/Preview son structs sueltas en el PaneVm.
// `sync_rows` (barato, en cada tick) actualiza el contenido. `sync_layout` (estructural)
// reconcilia la lista de paneles y splitters.
mod bridge;
mod config_ctrl;
mod devices;
mod i18n_keys;
mod icons;
mod keys;
mod listing;
mod logging;
mod ops_ctrl;
mod packs;
mod preview;
mod theme_apply;
mod tray;
mod watch;
mod workspace_ctrl;

use naygo_core::workspace::layout::{Rect, SplitDir};
use naygo_core::workspace::{PaneId, PanePurpose};
use slint::{Model, ModelRc, SharedPixelBuffer, SharedString, TimerMode, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use workspace_ctrl::WorkspaceCtrl;

slint::include_modules!();

/// CHANGELOG embebido en build time: fuente de la sección "Novedades" del Acerca de.
/// Ruta relativa desde este archivo (crates/ui-slint/src/) hasta la raíz del repo.
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// Modelos de lista ESTABLES de un panel (solo el que aplica a su tipo se usa).
struct PaneModels {
    rows: Rc<VecModel<RowData>>,
    /// Columnas visibles del panel Files (6C): se actualizan in situ como las filas.
    columns: Rc<VecModel<ColumnVm>>,
    /// TODAS las columnas (para el menú agregar/quitar) (6C).
    col_menu: Rc<VecModel<ColumnToggleVm>>,
    tree: Rc<VecModel<TreeRow>>,
    favs: Rc<VecModel<NavRow>>,
    recents: Rc<VecModel<NavRow>>,
    /// Árbol de favoritos editable (panel Favoritos): grupos + hojas aplanados con sangría.
    fav_tree: Rc<VecModel<FavTreeRow>>,
    hist: Rc<VecModel<HistRow>>,
}

impl PaneModels {
    fn new() -> PaneModels {
        PaneModels {
            rows: Rc::new(VecModel::default()),
            columns: Rc::new(VecModel::default()),
            col_menu: Rc::new(VecModel::default()),
            tree: Rc::new(VecModel::default()),
            favs: Rc::new(VecModel::default()),
            recents: Rc::new(VecModel::default()),
            fav_tree: Rc::new(VecModel::default()),
            hist: Rc::new(VecModel::default()),
        }
    }
}

/// Modelos estables que persisten entre refrescos (ver nota de cabecera).
struct Models {
    panes: Rc<VecModel<PaneVm>>,
    splits: Rc<VecModel<SplitVm>>,
    /// Candidatos del selector de panel destino (vacío = sin selector).
    picks: Rc<VecModel<PickVm>>,
    /// Modelos de lista estables por panel (se actualizan in situ, no se recrean).
    per_pane: HashMap<PaneId, PaneModels>,
    /// IDs de panel VISIBLES en el orden actual del modelo `panes`.
    pane_ids: Vec<PaneId>,
    /// Grupos de pestañas con los que se construyó la estructura (para detectar cambios de
    /// agrupación que no alteran los ids visibles, p. ej. activar otra pestaña).
    groups: Vec<(Vec<PaneId>, usize)>,
    /// Área con la que se construyó la estructura actual (para detectar resize).
    area: Rect,
}

impl Models {
    fn new() -> Models {
        Models {
            panes: Rc::new(VecModel::default()),
            splits: Rc::new(VecModel::default()),
            picks: Rc::new(VecModel::default()),
            per_pane: HashMap::new(),
            pane_ids: Vec::new(),
            groups: Vec::new(),
            area: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
        }
    }

    fn models_for(&mut self, id: PaneId) -> &PaneModels {
        self.per_pane.entry(id).or_insert_with(PaneModels::new)
    }
}

fn rects_equal(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < 0.5
        && (a.y - b.y).abs() < 0.5
        && (a.w - b.w).abs() < 0.5
        && (a.h - b.h).abs() < 0.5
}

fn purpose_to_int(p: PanePurpose) -> i32 {
    match p {
        PanePurpose::Files => 0,
        PanePurpose::Tree => 1,
        PanePurpose::Inspector => 2,
        PanePurpose::History => 3,
        PanePurpose::Favorites => 4,
        PanePurpose::Preview => 5,
        PanePurpose::Operations => 6,
    }
}

/// Offset del huso local en minutos (positivo al este de UTC). Delega en
/// `naygo_platform::time::local_utc_offset_secs` que ya usa GetTimeZoneInformation
/// con el feature Win32_System_Time activo en el crate platform.
fn win_tz_offset_minutes() -> i32 {
    (naygo_platform::time::local_utc_offset_secs() / 60) as i32
}

/// Cadena corta del SO para el log de entorno.
#[cfg(windows)]
fn os_version_string() -> String {
    "Windows".to_string()
}
#[cfg(not(windows))]
fn os_version_string() -> String {
    std::env::consts::OS.to_string()
}

/// Texto de ayuda para `--help`: uso de la línea de comandos en español neutral.
fn cli_help_text() -> String {
    "Uso: naygo.exe [carpeta] [opciones]\n\
     \n\
     [carpeta]          Abre esa carpeta en el panel activo.\n\
     --theme <id>       Usa ese tema solo en esta ejecución (no se guarda).\n\
     --layout <nombre>  Usa esa plantilla de disposición solo en esta ejecución.\n\
     --help             Muestra esta ayuda y sale.\n\
     --version          Muestra la versión y sale."
        .to_string()
}

fn main() -> Result<(), slint::PlatformError> {
    // Logging a archivo + panic handler ANTES de todo: una caída se registra y se avisa con un
    // diálogo, en vez de cerrarse en silencio (el log queda en naygo.log junto al ejecutable).
    // Offset del huso local (minutos) ANTES de init(): el nombre del archivo de log lleva la
    // fecha local del día (naygo-YYYY-MM-DD.log), así que el huso debe estar fijado antes de
    // resolver la ruta. Si falla, queda en UTC.
    crate::logging::set_tz_offset(win_tz_offset_minutes());
    // Logging a archivo + panic handler ANTES de todo: una caída se registra y se avisa con un
    // diálogo, en vez de cerrarse en silencio (el log queda junto al ejecutable).
    logging::init();

    // Render por SOFTWARE forzado en código (no por variable de entorno). Naygo no debe
    // depender de GPU: en VMs/equipos sin GPU el backend acelerado de Slint dejaba la ventana
    // en 0x0 y producía geometría no finita que reventaba en `euclid::Vector2D::cast` (panic
    // `Option::unwrap() on None`). El renderizador por software es estable en todos lados y
    // encaja con la premisa de bajo consumo. Se hace ANTES de crear cualquier ventana (splash
    // incluido). Si falla (otro backend ya activo, etc.), se registra y se sigue con el
    // backend por defecto — no tumbamos el arranque por esto.
    match i_slint_backend_winit::Backend::new_with_renderer_by_name(Some("software")) {
        Ok(backend) => match slint::platform::set_platform(Box::new(backend)) {
            Ok(()) => logging::log_line("backend: winit-software fijado OK"),
            Err(e) => logging::log_line(&format!(
                "No se pudo fijar el backend software (set_platform): {e}"
            )),
        },
        Err(e) => {
            logging::log_line(&format!("No se pudo crear el backend winit-software: {e}"));
        }
    }

    // Argumentos de línea de comandos: el core ya los parsea (carpeta a abrir, --theme,
    // --layout, --help, --version). `--help`/`--version` muestran un diálogo y NO abren la
    // ventana; el resto se aplica más abajo, una vez construido el controlador.
    let cli_args = naygo_core::cli::parse_args_real(&std::env::args().skip(1).collect::<Vec<_>>());
    if cli_args.help {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Info)
            .set_title("Naygo — opciones")
            .set_description(cli_help_text())
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
        return Ok(());
    }
    if cli_args.version {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Info)
            .set_title("Naygo")
            .set_description(format!(
                "Naygo v{}\nNicolás Groth / ISGroth · MIT",
                env!("CARGO_PKG_VERSION")
            ))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
        return Ok(());
    }

    let ui = AppWindow::new()?;
    let start = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:/"));
    let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new(start)));
    // Estado de la paleta de comandos (Ctrl+P): la lista de comandos vigente mientras está
    // abierta (la arma `build_palette_commands` al abrir) y, en paralelo, el índice del COMANDO
    // que cada FILA visible ejecuta (lo llena `palette_items_from_matches`). El callback
    // `on_palette_run(result_idx)` traduce fila→comando con esta tabla.
    let palette_cmds: Rc<RefCell<Vec<naygo_core::palette::Command>>> =
        Rc::new(RefCell::new(Vec::new()));
    let palette_cmd_indices: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    // Restaurar la sesión anterior (paneles y carpetas) si hay una guardada. Si NO hay sesión
    // previa (primera ejecución), arrancar con la disposición clásica: árbol + dos paneles de
    // archivos + Propiedades + Vista previa, en vez del panel único de arranque. Las sesiones
    // guardadas se respetan (solo se aplica el clásico cuando load_session no restauró nada).
    if !ctrl.borrow_mut().load_session() {
        ctrl.borrow_mut().apply_first_run_layout();
    }
    // Disponibilidad de terminales opcionales (Windows Terminal / WSL): se consulta una vez al
    // arranque (escanea el PATH) para decidir qué entradas mostrar en el combo de la toolbar.
    ui.set_has_wt(ctrl.borrow().windows_terminal_available());
    ui.set_has_wsl(ctrl.borrow().wsl_available());
    // Volcar los textos del idioma activo al global Tr (la UI arranca traducida).
    i18n_keys::apply(&ui, &ctrl.borrow().config);
    // Volcar los colores del tema activo al global Theme (la UI arranca con el tema guardado).
    theme_apply::apply(&ui, ctrl.borrow().config.active_theme());

    // Ventana de configuración: ahora es una ventana nativa propia (antes era un overlay dentro
    // de la AppWindow). Se construye una sola vez y vive en el mismo bucle de eventos que la
    // ventana principal (como el Splash). Se muestra/oculta con el botón del engranaje y se cierra
    // con su callback `close` o con la X del sistema. Cada ventana Slint tiene su PROPIA copia de
    // los globales `Theme`/`Tr`, así que hay que aplicarle el tema y el idioma por separado.
    let cfg_win = Rc::new(ConfigWindow::new()?);
    i18n_keys::apply(&*cfg_win, &ctrl.borrow().config);
    theme_apply::apply(&*cfg_win, ctrl.borrow().config.active_theme());

    // Aplicar los argumentos de CLI ahora que el controlador y ambas ventanas existen. Orden:
    // layout (dispone los paneles) → carpeta (navega el panel activo resultante) → tema (repinta
    // ambas ventanas). Todo es para ESTA sesión: --theme/--layout NO persisten ni la carpeta
    // entra como tema/plantilla por defecto. Los problemas (plantilla/tema/ruta inválidos) se
    // juntan en `avisos`; la app abre igual.
    {
        let mut avisos: Vec<String> = Vec::new();
        // 1) --layout: aplica la plantilla por nombre SIN persistir (built-in + usuario).
        if let Some(name) = cli_args.layout.as_deref() {
            if !ctrl.borrow_mut().apply_template_ephemeral(name) {
                avisos.push(format!("La plantilla \"{name}\" no existe; se ignoró."));
            }
        }
        // 2) carpeta: navega el panel Files activo de la disposición resultante. Manda sobre la
        // sesión restaurada/clásica (es un pedido explícito del usuario).
        if let Some(dir) = cli_args.dir.clone() {
            ctrl.borrow_mut().navigate_active_to(dir);
        } else if let Some(raw) = cli_args.dir_arg_raw.as_deref() {
            // 4) carpeta inválida: se pasó algo que no resolvió a una carpeta.
            avisos.push(format!("La ruta \"{raw}\" no es una carpeta; se ignoró."));
        }
        // 3) --theme: aplica el tema por id SOLO en memoria y repinta ambas ventanas.
        if let Some(id) = cli_args.theme.as_deref() {
            let aplicado = ctrl
                .borrow_mut()
                .config
                .set_theme_ephemeral(naygo_core::theme::ThemeId::new(id));
            if aplicado {
                let c = ctrl.borrow();
                theme_apply::apply(&ui, c.config.active_theme());
                theme_apply::apply(&*cfg_win, c.config.active_theme());
            } else {
                avisos.push(format!(
                    "El tema \"{id}\" no existe; se usó el predeterminado."
                ));
            }
        }
        // 5) avisos: al log siempre; un diálogo solo si hubo alguno. La app abre igual.
        for a in &avisos {
            crate::logging::log_line(&format!("CLI: {a}"));
        }
        if !avisos.is_empty() {
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title("Naygo — argumentos")
                .set_description(avisos.join("\n"))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
        }
    }

    // La X del sistema oculta la ventana (no cierra la app ni destruye la instancia): así se
    // reabre con el estado intacto.
    {
        let cfg_weak = cfg_win.as_weak();
        cfg_win.window().on_close_requested(move || {
            if let Some(w) = cfg_weak.upgrade() {
                let _ = w.hide();
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    // Splash de arranque (Fase 5F): solo en release. Ventana breve de bienvenida que se cierra
    // sola a ~1.2s. La ventana principal se construye por detrás (el splash no la bloquea). En
    // debug se omite (arranque directo). Se mantiene vivo en una variable de la función `main`.
    // Nota: el Splash usa los colores POR DEFECTO del global Theme (azul marino), que coinciden
    // con el tema default — no hace falta aplicarle el tema activo (es una pantalla efímera).
    #[cfg(not(debug_assertions))]
    let _splash_keepalive = match Splash::new() {
        Ok(splash) => {
            let _ = splash.show();
            let splash = Rc::new(splash);
            let splash_for_timer = splash.clone();
            let timer = slint::Timer::default();
            timer.start(
                slint::TimerMode::SingleShot,
                std::time::Duration::from_millis(1200),
                move || {
                    let _ = splash_for_timer.hide();
                },
            );
            Some((splash, timer))
        }
        Err(_) => None,
    };

    let models = Rc::new(RefCell::new(Models::new()));

    ui.set_panes(ModelRc::from(models.borrow().panes.clone()));
    ui.set_splits(ModelRc::from(models.borrow().splits.clone()));
    ui.set_picks(ModelRc::from(models.borrow().picks.clone()));

    let area_of = {
        let ui_weak = ui.as_weak();
        move || {
            ui_weak
                .upgrade()
                .map(|ui| Rect {
                    x: 0.0,
                    y: 0.0,
                    w: ui.get_content_w().max(0.0),
                    h: ui.get_content_h().max(0.0),
                })
                .unwrap_or(Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                })
        }
    };

    // Actualiza SOLO el contenido (filas + structs + flags) sin tocar la estructura. Barato:
    // corre en cada tick. Mantiene los mismos VecModel → los ListView conservan su scroll.
    let sync_rows = {
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        let models = models.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            // `borrow_mut` porque `rows_of` necesita mutar el IconCache (decodifica on-demand).
            let mut c = ctrl.borrow_mut();
            let active = c.active_id();
            let hl_secs = c.highlight_secs();
            let hl_now = std::time::Instant::now();
            // Datos compartidos (no dependen del panel concreto): favoritos, recientes,
            // historial, inspector, preview se derivan del estado global / panel activo.
            let favs: Vec<NavRow> = c.favorite_rows().into_iter().map(to_nav_row).collect();
            let fav_tree: Vec<FavTreeRow> =
                c.fav_tree_rows().into_iter().map(to_fav_tree_row).collect();
            // Grupos para el submenú "Mover a…" del panel y para que el menú ▾ del toolbar exista.
            let fav_group_options: Vec<PathSeg> = c
                .fav_group_options()
                .into_iter()
                .map(|(label, gid)| PathSeg {
                    label: SharedString::from(label),
                    path: SharedString::from(gid),
                })
                .collect();
            let recents: Vec<NavRow> = c.recent_rows().into_iter().map(to_nav_row).collect();
            let hist: Vec<HistRow> = c.history_rows().into_iter().map(to_hist_row).collect();
            let inspector = to_inspector_vm(c.inspector_info());

            // Props a nivel de ventana para el menú ▾ de favoritos del toolbar (el árbol jerárquico)
            // y para el submenú "Mover a…" del panel (lista de grupos destino).
            ui.set_fav_tree(ModelRc::from(Rc::new(VecModel::from(fav_tree.clone()))));
            ui.set_fav_group_options(ModelRc::from(Rc::new(VecModel::from(
                fav_group_options.clone(),
            ))));

            let mut m = models.borrow_mut();
            for (i, &id) in m.pane_ids.clone().iter().enumerate() {
                let purpose = c.purpose_of(id);
                // Actualiza los modelos de lista que apliquen al tipo, in situ.
                match purpose {
                    Some(PanePurpose::Files) => {
                        let rows: Vec<RowData> = c
                            .rows_of(id, hl_secs, hl_now)
                            .into_iter()
                            .map(to_row_data)
                            .collect();
                        let cols: Vec<ColumnVm> =
                            c.columns_of(id).into_iter().map(to_column_vm).collect();
                        let col_menu: Vec<ColumnToggleVm> = c
                            .column_toggles_of(id)
                            .into_iter()
                            .map(to_column_toggle_vm)
                            .collect();
                        let pm = m.models_for(id);
                        pm.rows.set_vec(rows);
                        pm.columns.set_vec(cols);
                        pm.col_menu.set_vec(col_menu);
                    }
                    Some(PanePurpose::Tree) => {
                        let rows: Vec<TreeRow> =
                            c.tree_rows(id).into_iter().map(to_tree_row).collect();
                        m.models_for(id).tree.set_vec(rows);
                    }
                    Some(PanePurpose::Favorites) => {
                        m.models_for(id).favs.set_vec(favs.clone());
                        m.models_for(id).recents.set_vec(recents.clone());
                        m.models_for(id).fav_tree.set_vec(fav_tree.clone());
                    }
                    Some(PanePurpose::History) => {
                        m.models_for(id).hist.set_vec(hist.clone());
                    }
                    _ => {}
                }
                // Actualiza los campos del PaneVm sin recrear el elemento.
                if let Some(mut pv) = m.panes.row_data(i) {
                    let is_active = Some(id) == active;
                    let path = SharedString::from(c.path_of(id).as_str());
                    let mut changed = false;
                    if pv.active != is_active {
                        pv.active = is_active;
                        changed = true;
                    }
                    if pv.path != path {
                        pv.path = path;
                        // Los breadcrumbs y el título dependen de la carpeta: hay que
                        // reconstruirlos al navegar (antes solo se armaban en sync_layout, así
                        // que el contenido cambiaba pero los segmentos quedaban viejos).
                        let segs: Vec<PathSeg> = c
                            .path_segments_of(id)
                            .into_iter()
                            .map(|(label, path)| PathSeg {
                                label: SharedString::from(label.as_str()),
                                path: SharedString::from(path.as_str()),
                            })
                            .collect();
                        pv.segments = ModelRc::from(Rc::new(VecModel::from(segs)));
                        pv.title = SharedString::from(c.pane_label(id).as_str());
                        changed = true;
                    }
                    // La estrella de favorito puede cambiar al navegar o al togglear.
                    if purpose == Some(PanePurpose::Files) {
                        let fav = c.is_pane_dir_favorite(id);
                        if pv.is_favorite != fav {
                            pv.is_favorite = fav;
                            changed = true;
                        }
                        // Aviso "sin coincidencias": filtro activo que vació la vista (F2).
                        let nm = c.no_matches(id);
                        if pv.no_matches != nm {
                            pv.no_matches = nm;
                            changed = true;
                        }
                        // Aviso "carpeta no encontrada": hay que refrescarlo en cada tick (no solo
                        // en sync_layout). Sin esto, al "subir nivel" / "elegir otra" / "reintentar"
                        // el panel navegaba a una carpeta válida pero el campo `missing` quedaba
                        // pegado en true y el aviso seguía tapando el listado.
                        let miss = c.pane_dir_missing(id);
                        if pv.missing != miss {
                            pv.missing = miss;
                            changed = true;
                        }
                        let miss_path = SharedString::from(c.path_of(id).as_str());
                        if pv.missing_path != miss_path {
                            pv.missing_path = miss_path;
                            changed = true;
                        }
                        let miss_anc = c.pane_has_existing_ancestor(id);
                        if pv.missing_has_ancestor != miss_anc {
                            pv.missing_has_ancestor = miss_anc;
                            changed = true;
                        }
                        // Estado del botón de vista profunda: on/off según el job activo.
                        let deep = c.is_deep_active(id);
                        if pv.deep_active != deep {
                            pv.deep_active = deep;
                            changed = true;
                        }
                        // Fila enfocada (índice de vista): al navegar por teclado cambia `f.focused`
                        // y la UI (changed focused-row) arrastra el scroll para revelarla (C1). Va
                        // en el refresco central para cubrir TODO lo que mueve el foco (↑↓, Re/Av
                        // Pág, Inicio/Fin, typeahead, saltar desde la paleta), no solo un atajo.
                        let focused = c.focused_view_of(id);
                        if pv.focused_row != focused {
                            pv.focused_row = focused;
                            changed = true;
                        }
                        // Footer (barra inferior): selección + disco. Vacío si está deshabilitado.
                        // El disco se cachea por unidad dentro del controlador (no pega a WinAPI
                        // en cada tick).
                        let footer = SharedString::from(c.footer_text_of(id).as_str());
                        if pv.footer_text != footer {
                            pv.footer_text = footer;
                            changed = true;
                        }
                    }
                    // Inspector/Preview son structs sueltas: se setean según el tipo.
                    if purpose == Some(PanePurpose::Inspector) {
                        pv.inspector = inspector.clone();
                        changed = true;
                    }
                    if purpose == Some(PanePurpose::Preview) {
                        pv.preview = current_preview_vm(&c);
                        changed = true;
                    }
                    if changed {
                        m.panes.set_row_data(i, pv);
                    }
                }
            }
            if let Some(id) = active {
                ui.set_active_path(SharedString::from(c.path_of(id).as_str()));
            }
            ui.set_status(SharedString::from(c.status_line().as_str()));
            // Botones Atrás/Adelante del toolbar: habilitados según el historial del panel activo.
            // Va aquí (refresco central) para que se actualicen tras cualquier navegación —teclado,
            // mouse, doble-clic, breadcrumbs— no solo al pulsar los botones.
            ui.set_can_go_back(c.can_go_back());
            ui.set_can_go_forward(c.can_go_forward());
            // Casillas del menú del "ojo" (visibilidad): reflejan los settings. Refresco central
            // para que queden al día tras alternar, factory-reset o recarga de config.
            ui.set_show_hidden(c.config.settings.show_hidden);
            ui.set_show_system(c.config.settings.show_system);
            ui.set_hide_dotfiles(c.config.settings.hide_dotfiles);
            // Operaciones de archivo (F3): modal activo + filas de progreso + retomar.
            ui.set_op_dialog(to_op_dialog_vm(c.ops.dialog_vm()));
            let op_rows: Vec<OpRowVm> = c.ops.op_rows().into_iter().map(to_op_row_vm).collect();
            // El panel rico de operaciones consume modelos separados por zona (kind: 0=en curso
            // 1=en cola 2=historial 3=calculando). Separarlos en Rust evita filas-fantasma en Slint.
            let running: Vec<OpRowVm> = op_rows.iter().filter(|r| r.kind == 0).cloned().collect();
            let queued: Vec<OpRowVm> = op_rows.iter().filter(|r| r.kind == 1).cloned().collect();
            let history: Vec<OpRowVm> = op_rows.iter().filter(|r| r.kind == 2).cloned().collect();
            let planning: Vec<OpRowVm> = op_rows.iter().filter(|r| r.kind == 3).cloned().collect();
            ui.set_op_running_count(running.len() as i32);
            ui.set_op_queued_count(queued.len() as i32);
            ui.set_op_history_count(history.len() as i32);
            ui.set_op_planning_count(planning.len() as i32);
            ui.set_op_running_rows(ModelRc::from(Rc::new(VecModel::from(running))));
            ui.set_op_queued_rows(ModelRc::from(Rc::new(VecModel::from(queued))));
            ui.set_op_history_rows(ModelRc::from(Rc::new(VecModel::from(history))));
            ui.set_op_planning_rows(ModelRc::from(Rc::new(VecModel::from(planning))));
            let resume_rows: Vec<ResumeRowVm> = c
                .ops
                .resume_rows()
                .into_iter()
                .map(|(id, label)| ResumeRowVm {
                    id: SharedString::from(id.as_str()),
                    label: SharedString::from(label.as_str()),
                })
                .collect();
            ui.set_resume_rows(ModelRc::from(Rc::new(VecModel::from(resume_rows))));
            // Menú contextual: posición + si hay menú nativo disponible (hay HWND).
            let ctx = match &c.context_menu {
                Some(cm) => ContextMenuVm {
                    active: true,
                    x: cm.x,
                    y: cm.y,
                    has_native: naygo_hwnd(&ui).is_some(),
                    has_wt: c.windows_terminal_available(),
                    folder_mode: cm.folder_mode,
                },
                None => ContextMenuVm {
                    active: false,
                    x: 0.0,
                    y: 0.0,
                    has_native: false,
                    has_wt: false,
                    folder_mode: false,
                },
            };
            ui.set_ctx_menu(ctx);
            // Modal "nueva(s) carpeta(s)".
            let (nf_valid, _nf_invalid) = c.new_folder_counts();
            ui.set_new_folder_vm(NewFolderVm {
                active: c.new_folder_open(),
                dir: c.new_folder_dir().into(),
                text: c.new_folder_text().into(),
                status: c.new_folder_status().into(),
                can_create: nf_valid > 0,
            });
            // (El aviso "carpeta no encontrada" es ahora IN-PLACE por panel: se arma en el PaneVm
            //  con `missing`/`missing-path`; ya no hay un VM de modal global.)
            // Panel de búsqueda recursiva (Ctrl+F / lupa).
            {
                let open = c.search_open();
                let (status, running) = c.search_status_text();
                let root_label = c.search_root_label();
                let query = c.search_query();
                let hits: Vec<SearchHitVm> = c
                    .search_rows()
                    .into_iter()
                    .map(|r| SearchHitVm {
                        name: r.name.into(),
                        rel_dir: r.rel_dir.into(),
                        detail: r.detail.into(),
                        is_dir: r.is_dir,
                        icon: r.icon,
                    })
                    .collect();
                ui.set_search_vm(SearchVm {
                    active: open,
                    query: query.into(),
                    root_label: root_label.into(),
                    running,
                    hits: ModelRc::new(VecModel::from(hits)),
                    status: status.into(),
                });
            }
            // Menú/editor de columna (clic derecho en el header, F2).
            let colmenu = match c.column_menu_snapshot() {
                Some(m) => {
                    let no_ext = ui.global::<Tr>().get_colfilter_no_ext();
                    let exts: Vec<ExtRowVm> = m
                        .exts
                        .into_iter()
                        .map(|e| {
                            let label = if e.ext.is_empty() {
                                no_ext.clone()
                            } else {
                                SharedString::from(e.ext.as_str())
                            };
                            ExtRowVm {
                                ext: SharedString::from(e.ext.as_str()),
                                label,
                                count: e.count as i32,
                                checked: e.checked,
                            }
                        })
                        .collect();
                    ColumnMenuVm {
                        active: true,
                        x: m.x,
                        y: m.y,
                        kind: m.kind,
                        label: SharedString::from(m.label.as_str()),
                        mode: m.mode,
                        has_filter: m.has_filter,
                        can_hide: m.can_hide,
                        text_draft: SharedString::from(m.text_draft.as_str()),
                        text_case: m.text_case,
                        min_draft: SharedString::from(m.min_draft.as_str()),
                        max_draft: SharedString::from(m.max_draft.as_str()),
                        exts: ModelRc::from(Rc::new(VecModel::from(exts))),
                    }
                }
                None => ColumnMenuVm {
                    active: false,
                    x: 0.0,
                    y: 0.0,
                    kind: 0,
                    label: SharedString::new(),
                    mode: 0,
                    has_filter: false,
                    can_hide: false,
                    text_draft: SharedString::new(),
                    text_case: false,
                    min_draft: SharedString::new(),
                    max_draft: SharedString::new(),
                    exts: ModelRc::from(Rc::new(VecModel::<ExtRowVm>::default())),
                },
            };
            ui.set_column_menu(colmenu);

            // Ventana de renombrado por lotes (F5): espejo del estado + preview en vivo.
            let batch = match &c.batch {
                Some(b) => {
                    use naygo_core::batch_rename::{CaseTransform, RowStatus};
                    let rows_src = c.batch_preview();
                    let rows: Vec<BatchRowVm> = rows_src
                        .iter()
                        .map(|r| BatchRowVm {
                            old_name: SharedString::from(r.old_name.as_str()),
                            new_name: SharedString::from(r.new_name.as_str()),
                            status: match r.status {
                                RowStatus::Ok => 0,
                                RowStatus::Unchanged => 1,
                                RowStatus::Invalid(_) => 2,
                                RowStatus::Collision => 3,
                            },
                        })
                        .collect();
                    BatchRenameVm {
                        active: true,
                        template: SharedString::from(b.spec.template.as_str()),
                        find: SharedString::from(b.spec.find.as_str()),
                        replace: SharedString::from(b.spec.replace.as_str()),
                        use_regex: b.spec.use_regex,
                        include_ext: b.spec.include_ext,
                        case: match b.spec.case {
                            CaseTransform::None => 0,
                            CaseTransform::Lower => 1,
                            CaseTransform::Upper => 2,
                            CaseTransform::Title => 3,
                        },
                        counter_start: SharedString::from(b.spec.counter_start.to_string()),
                        counter_step: SharedString::from(b.spec.counter_step.to_string()),
                        count: b.items.len() as i32,
                        can_apply: c.batch_can_apply(),
                        rows: ModelRc::from(Rc::new(VecModel::from(rows))),
                    }
                }
                None => BatchRenameVm {
                    active: false,
                    template: SharedString::new(),
                    find: SharedString::new(),
                    replace: SharedString::new(),
                    use_regex: false,
                    include_ext: false,
                    case: 0,
                    counter_start: SharedString::new(),
                    counter_step: SharedString::new(),
                    count: 0,
                    can_apply: false,
                    rows: ModelRc::from(Rc::new(VecModel::<BatchRowVm>::default())),
                },
            };
            ui.set_batch(batch);

            // Ayuda (F1): estado + atajos activos (leídos del keymap en vivo).
            ui.set_help_open(c.help_open);
            let help_rows: Vec<HelpRowVm> = c
                .help_shortcuts()
                .into_iter()
                .map(|(label, chord)| HelpRowVm {
                    label: SharedString::from(label.as_str()),
                    chord: SharedString::from(chord.as_str()),
                })
                .collect();
            ui.set_help_shortcuts(ModelRc::from(Rc::new(VecModel::from(help_rows))));
            crate::logging::set_diag_snapshot(c.diag_snapshot());
        }
    };

    // Reconcilia la ESTRUCTURA (paneles + splitters) con el estado del core. Solo
    // reconstruye cuando cambia la lista de IDs o el área. Tras reestructurar, sincroniza.
    let sync_layout: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let models = models.clone();
        let area_of = area_of.clone();
        let sync_rows = sync_rows.clone();
        Rc::new(move || {
            let area = area_of();
            ctrl.borrow_mut().set_area(area);
            let pane_rects = ctrl.borrow().pane_rects(area);
            let split_handles = ctrl.borrow().split_handles(area);
            // Grupos de pestañas: solo se PINTA la pestaña activa de cada grupo (todas
            // comparten rect). Los miembros ocultos se filtran; al activo se le adjunta la
            // lista de pestañas para que pinte la barra.
            let groups = ctrl.borrow().tab_groups();
            let grouped: std::collections::HashSet<PaneId> =
                groups.iter().flat_map(|(m, _)| m.iter().copied()).collect();
            let active_members: std::collections::HashSet<PaneId> = groups
                .iter()
                .filter_map(|(m, a)| m.get(*a).copied())
                .collect();
            // Rects visibles: panel no agrupado, o la pestaña activa de su grupo.
            let visible: Vec<(PaneId, Rect)> = pane_rects
                .iter()
                .filter(|(id, _)| !grouped.contains(id) || active_members.contains(id))
                .copied()
                .collect();
            let new_ids: Vec<PaneId> = visible.iter().map(|(id, _)| *id).collect();
            // Todos los ids del layout (visibles + ocultos) para conservar sus modelos.
            let all_ids: Vec<PaneId> = pane_rects.iter().map(|(id, _)| *id).collect();

            let mut m = models.borrow_mut();
            // La estructura cambió si cambió la lista visible, el área, o algún grupo
            // (apilar/activar pestaña no cambia los ids visibles pero sí las barras).
            let structure_changed =
                new_ids != m.pane_ids || !rects_equal(area, m.area) || groups != m.groups;

            if structure_changed {
                let active = ctrl.borrow().active_id();
                let panes: Vec<PaneVm> = visible
                    .iter()
                    .map(|(id, r)| {
                        let c = ctrl.borrow();
                        let purpose = c.purpose_of(*id).map(purpose_to_int).unwrap_or(0);
                        // Si este id es la pestaña activa de un grupo, armar su barra.
                        let tabs: Vec<TabVm> = groups
                            .iter()
                            .find(|(mem, a)| mem.get(*a) == Some(id))
                            .map(|(mem, a)| {
                                mem.iter()
                                    .enumerate()
                                    .map(|(i, mid)| TabVm {
                                        id: mid.0 as i32,
                                        label: SharedString::from(c.pane_label(*mid).as_str()),
                                        active: i == *a,
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        drop(c);
                        let pm = m.models_for(*id);
                        PaneVm {
                            id: id.0 as i32,
                            x: r.x,
                            y: r.y,
                            w: r.w,
                            h: r.h,
                            path: SharedString::from(ctrl.borrow().path_of(*id).as_str()),
                            active: Some(*id) == active,
                            purpose,
                            title: SharedString::from(ctrl.borrow().pane_label(*id).as_str()),
                            rows: ModelRc::from(pm.rows.clone()),
                            columns: ModelRc::from(pm.columns.clone()),
                            col_menu: ModelRc::from(pm.col_menu.clone()),
                            is_favorite: ctrl.borrow().is_pane_dir_favorite(*id),
                            no_matches: ctrl.borrow().no_matches(*id),
                            // Carpeta no encontrada / ilegible: aviso in-place con opciones.
                            missing: ctrl.borrow().pane_dir_missing(*id),
                            missing_path: SharedString::from(ctrl.borrow().path_of(*id).as_str()),
                            // "Subir un nivel" solo tiene sentido si hay un ancestro existente real.
                            missing_has_ancestor: ctrl.borrow().pane_has_existing_ancestor(*id),
                            // Fila enfocada (índice de vista) para el auto-scroll por teclado (C1).
                            focused_row: ctrl.borrow().focused_view_of(*id),
                            deep_active: ctrl.borrow().is_deep_active(*id),
                            // El footer se llena en el primer `sync_rows` (necesita `&mut` por la
                            // caché de disco). Aquí nace vacío.
                            footer_text: SharedString::new(),
                            segments: {
                                let segs: Vec<PathSeg> = ctrl
                                    .borrow()
                                    .path_segments_of(*id)
                                    .into_iter()
                                    .map(|(label, path)| PathSeg {
                                        label: SharedString::from(label.as_str()),
                                        path: SharedString::from(path.as_str()),
                                    })
                                    .collect();
                                ModelRc::from(Rc::new(VecModel::from(segs)))
                            },
                            tree_rows: ModelRc::from(pm.tree.clone()),
                            favs: ModelRc::from(pm.favs.clone()),
                            recents: ModelRc::from(pm.recents.clone()),
                            fav_tree: ModelRc::from(pm.fav_tree.clone()),
                            hist_rows: ModelRc::from(pm.hist.clone()),
                            inspector: InspectorVm::default(),
                            preview: PreviewVm::default(),
                            tabs: ModelRc::from(Rc::new(VecModel::from(tabs))),
                        }
                    })
                    .collect();
                m.panes.set_vec(panes);

                let splits: Vec<SplitVm> = split_handles
                    .iter()
                    .enumerate()
                    .map(|(i, h)| SplitVm {
                        index: i as i32,
                        x: h.rect.x,
                        y: h.rect.y,
                        w: h.rect.w,
                        h: h.rect.h,
                        horizontal: matches!(h.dir, SplitDir::Horizontal),
                    })
                    .collect();
                m.splits.set_vec(splits);

                m.per_pane.retain(|id, _| all_ids.contains(id));
                m.pane_ids = new_ids;
                m.groups = groups;
                m.area = area;
            } else {
                // La estructura NO cambió (mismos paneles, misma área, mismos grupos), pero los
                // RECTS pueden haber cambiado al arrastrar un splitter (cambia la fraction, no los
                // ids). Antes esto quedaba fuera del rebuild y el resize "no hacía nada". Ahora se
                // actualizan los x/y/w/h de cada PaneVm IN SITU (sin recrear modelos → conserva el
                // scroll) y se reposicionan las barras de splitter.
                for (id, r) in &visible {
                    if let Some(i) = m.pane_ids.iter().position(|p| p == id) {
                        if let Some(mut pv) = m.panes.row_data(i) {
                            if pv.x != r.x || pv.y != r.y || pv.w != r.w || pv.h != r.h {
                                pv.x = r.x;
                                pv.y = r.y;
                                pv.w = r.w;
                                pv.h = r.h;
                                m.panes.set_row_data(i, pv);
                            }
                        }
                    }
                }
                let splits: Vec<SplitVm> = split_handles
                    .iter()
                    .enumerate()
                    .map(|(i, h)| SplitVm {
                        index: i as i32,
                        x: h.rect.x,
                        y: h.rect.y,
                        w: h.rect.w,
                        h: h.rect.h,
                        horizontal: matches!(h.dir, SplitDir::Horizontal),
                    })
                    .collect();
                m.splits.set_vec(splits);
            }

            // Selector de panel destino: rect de cada candidato (orden visual) + su número.
            // Se reconstruye siempre (puede aparecer/desaparecer sin cambio de estructura).
            let picks: Vec<PickVm> = {
                let c = ctrl.borrow();
                match &c.pending_pick {
                    Some(pick) => {
                        let rects: std::collections::HashMap<PaneId, Rect> =
                            c.pane_rects(area).into_iter().collect();
                        pick.candidates
                            .iter()
                            .enumerate()
                            .filter_map(|(i, id)| {
                                rects.get(id).map(|r| PickVm {
                                    x: r.x,
                                    y: r.y,
                                    w: r.w,
                                    h: r.h,
                                    number: (i + 1) as i32,
                                })
                            })
                            .collect()
                    }
                    None => Vec::new(),
                }
            };
            m.picks.set_vec(picks);

            drop(m);
            sync_rows();
        })
    };

    // Waker para los watchers (carpeta/dispositivos): desde su hilo, encolan en el event loop
    // de Slint una llamada a `wake()` de la ventana (re-arranca el timer si dormía).
    // `slint::Weak` es Send; el closure del event loop corre en el hilo de UI.
    let waker: naygo_platform::dir_watch::Waker = {
        let ui_weak = ui.as_weak();
        std::sync::Arc::new(move || {
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_wake();
                }
            });
        })
    };

    // Watcher de dispositivos (Fase 5B): detecta USB enchufado/quitado. Vive toda la sesión.
    let devices = Rc::new(devices::Devices::start(waker.clone()));
    // HOME para reubicar paneles cuya unidad desapareció.
    let home: Rc<std::path::PathBuf> = Rc::new(
        std::env::var_os("USERPROFILE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("C:/")),
    );

    // Aplica un cambio de unidades (USB enchufado/quitado): drena el watcher, reubica paneles
    // huérfanos y RECONSTRUYE la tira de discos de la toolbar. Se llama desde el timer Y desde
    // `on_wake` para que un USB recién conectado aparezca EN VIVO aunque el timer esté dormido
    // (antes el refresh solo ocurría en el tick, que se apaga en reposo, y el USB no salía hasta
    // interactuar/reabrir). Idempotente: si no hubo cambios reales, no hace nada.
    let apply_device_change: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let devices = devices.clone();
        let home = home.clone();
        let ui_weak = ui.as_weak();
        Rc::new(move || {
            if !devices.drives_changed() {
                return;
            }
            // El espacio en disco pudo cambiar (USB conectado/expulsado): invalida la caché del
            // footer para que se relea en el próximo tick, igual que se refresca la tira de discos.
            ctrl.borrow_mut().invalidate_footer_disk_cache();
            let moved = ctrl.borrow_mut().relocate_orphans(&home);
            for id in moved {
                let dir = ctrl
                    .borrow()
                    .ws
                    .pane(id)
                    .and_then(|p| p.files.as_ref())
                    .map(|f| f.current_dir.clone());
                if let Some(dir) = dir {
                    ctrl.borrow_mut().start_listing(id, dir);
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                let drives: Vec<NavRow> = ctrl
                    .borrow_mut()
                    .drive_strip()
                    .into_iter()
                    .map(to_nav_row)
                    .collect();
                ui.set_drives(ModelRc::from(Rc::new(VecModel::from(drives))));
            }
        })
    };

    // Drag&drop OLE — RECIBIR (Fase 5D): canal de archivos soltados sobre la ventana. El
    // registro del IDropTarget se hace en el primer tick (cuando el HWND ya es válido) y el
    // guard vive toda la sesión. Por simplicidad el drop va al panel ACTIVO (fallback del
    // diseño: no se mapea el punto de drop a un panel concreto).
    let (drop_tx, drop_rx) = std::sync::mpsc::channel::<naygo_platform::drop_target::DropPayload>();
    let drop_rx = Rc::new(drop_rx);
    let drop_guard: Rc<RefCell<Option<naygo_platform::drop_target::DropTargetGuard>>> =
        Rc::new(RefCell::new(None));

    // Tray (Fase 5E): ícono en bandeja con menú Abrir/Salir, solo si el ajuste lo pide. Vive
    // toda la sesión. `tray_active` lo lee el handler de cierre para decidir si oculta a la
    // bandeja o sale de verdad.
    let tray: Rc<Option<tray::Tray>> = Rc::new(if ctrl.borrow().config.settings.tray_enabled {
        let t = {
            let c = ctrl.borrow();
            tray::create(
                &c.config.t("slint.tray.open"),
                &c.config.t("slint.tray.new_pane"),
                &c.config.t("slint.tray.config"),
                &c.config.t("slint.tray.center"),
                &c.config.t("slint.tray.exit"),
                waker.clone(),
            )
        };
        t
    } else {
        None
    });
    let tray_active = tray.is_some();

    // Timer que drena listados de archivos + árbol + preview; se apaga cuando todo está en
    // reposo (0 trabajo). El preview cambia structs del PaneVm → en cada tick sync_rows.
    let timer = Rc::new(slint::Timer::default());
    let start_timer: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let timer = timer.clone();
        let apply_device_change = apply_device_change.clone();
        let waker = waker.clone();
        let ui_weak = ui.as_weak();
        let drop_tx = drop_tx.clone();
        let drop_rx = drop_rx.clone();
        let drop_guard = drop_guard.clone();
        let tray = tray.clone();
        Rc::new(move || {
            let ctrl = ctrl.clone();
            let sync_rows = sync_rows.clone();
            let sync_layout = sync_layout.clone();
            let timer2 = timer.clone();
            let waker = waker.clone();
            let apply_device_change = apply_device_change.clone();
            let ui_weak = ui_weak.clone();
            let drop_tx = drop_tx.clone();
            let drop_rx = drop_rx.clone();
            let drop_guard = drop_guard.clone();
            let tray = tray.clone();
            timer.start(
                TimerMode::Repeated,
                std::time::Duration::from_millis(30),
                move || {
                    let now = std::time::Instant::now();
                    // Cinturón de seguridad anti-reentrancia: el bucle modal de `DoDragDrop`
                    // (arrastre OLE hacia afuera) corre dentro del mismo hilo de UI y RE-ENTRA
                    // este timer mientras dura el arrastre. Si en ese momento ya hay un
                    // `borrow_mut` de `ctrl` vivo más arriba en la pila, repintar aquí
                    // reventaría con «already borrowed». Probamos con `try_borrow_mut`: si el
                    // controlador está prestado, SALTAMOS este tick por completo (el siguiente
                    // tick repinta; perder un frame de 30ms es invisible). El probe es solo de
                    // liveness: soltamos el préstamo de inmediato.
                    if ctrl.try_borrow_mut().is_err() {
                        return;
                    }
                    // Registrar el destino de drop OLE una sola vez, cuando el HWND ya es válido
                    // (primer tick con la ventana realizada). El guard vive toda la sesión.
                    if drop_guard.borrow().is_none() {
                        if let Some(ui) = ui_weak.upgrade() {
                            if let Some(hwnd) = naygo_hwnd(&ui) {
                                let g = naygo_platform::drop_target::register(
                                    hwnd,
                                    drop_tx.clone(),
                                    waker.clone(),
                                );
                                *drop_guard.borrow_mut() = Some(g);
                            }
                        }
                    }
                    // Drag&drop OLE — RECIBIR: archivos soltados sobre la ventana → copiar (o
                    // mover) a la carpeta del panel que está BAJO el cursor. El payload trae el
                    // punto del cursor en coords de PANTALLA (físicas); lo convertimos a coords de
                    // CONTENIDO (el mismo sistema que usa pane_rects/drop_hit) y enrutamos con
                    // `drop_at`. Sirve tanto para drags intra-app (entre paneles) como para drags
                    // desde el Explorador de Windows (ahora caen en el panel apuntado, no en el
                    // activo).
                    //
                    // Conversión pantalla→contenido:
                    //   cliente = ScreenToClient(hwnd, pantalla)    (físicos; el SO descuenta el
                    //             marco y la barra de título nativos de un golpe)
                    //   win_log = cliente / scale_factor            (a coords lógicas)
                    //   content = win_log - (0, TOP_BAR_H)          (descontar la barra superior
                    //             de Naygo, que SÍ vive dentro del área de cliente, sobre `content`)
                    // El área de contenido tiene origen (0,0) bajo la barra (ver app-window.slint:
                    // `content` empieza en y=TOP_BAR_H, x=0), así que en X no hay offset lateral.
                    // ScreenToClient ya quitó el marco del SO: NO restar nada más por ese lado.
                    // TOP_BAR_H = 34px lógicos (alto de la barra superior de Naygo).
                    const TOP_BAR_H: f32 = 34.0;
                    while let Ok(payload) = drop_rx.try_recv() {
                        let mut routed = false;
                        if let Some(ui) = ui_weak.upgrade() {
                            // Punto del drop en coords de CLIENTE (físicas) vía Win32. Si no hay
                            // HWND o ScreenToClient falla, caemos al fallback del panel activo.
                            let client = naygo_hwnd(&ui).and_then(|hwnd| {
                                naygo_platform::window::screen_to_client(
                                    hwnd,
                                    payload.screen_x,
                                    payload.screen_y,
                                )
                            });
                            if let Some((client_x, client_y)) = client {
                                let scale = ui.window().scale_factor().max(0.01);
                                let cx = client_x as f32 / scale;
                                let cy = client_y as f32 / scale - TOP_BAR_H;
                                let (ctrl_down, shift_down) = {
                                    let c = ctrl.borrow();
                                    (c.ctrl_down, c.shift_down)
                                };
                                routed = ctrl.borrow_mut().drop_at(
                                    cx,
                                    cy,
                                    ctrl_down,
                                    shift_down,
                                    payload.paths.clone(),
                                    payload.move_,
                                );
                            }
                        }
                        // Fallback: si no se pudo enrutar por el punto (sin ventana, o el cursor
                        // no cayó sobre un panel Files), caer al panel activo como antes para no
                        // perder el drop.
                        if !routed {
                            if let Some(active) = ctrl.borrow().active_id() {
                                ctrl.borrow_mut().drop_external(
                                    active,
                                    payload.paths,
                                    payload.move_,
                                );
                            }
                        }
                    }
                    // Tray (F5E): drenar los mensajes del ícono de bandeja. Abrir = mostrar y
                    // elevar la ventana; Salir = terminar el bucle de verdad.
                    if let Some(t) = tray.as_ref() {
                        while let Ok(msg) = t.rx.try_recv() {
                            match msg {
                                tray::TrayMsg::Open => {
                                    if let Some(ui) = ui_weak.upgrade() {
                                        let _ = ui.show();
                                        ui.window().set_minimized(false);
                                    }
                                }
                                tray::TrayMsg::NewPane => {
                                    // Traer al frente y abrir un panel nuevo (divide el activo).
                                    if let Some(ui) = ui_weak.upgrade() {
                                        let _ = ui.show();
                                        ui.window().set_minimized(false);
                                    }
                                    ctrl.borrow_mut().add_pane_split();
                                    sync_layout();
                                }
                                tray::TrayMsg::OpenConfig => {
                                    // Reusa el handler del engranaje de la toolbar (refresca el VM
                                    // y muestra la ventana de config), así no duplicamos lógica.
                                    if let Some(ui) = ui_weak.upgrade() {
                                        let _ = ui.show();
                                        ui.window().set_minimized(false);
                                        ui.invoke_open_config();
                                    }
                                }
                                tray::TrayMsg::CenterWindow => {
                                    // Rescatar una ventana "perdida": mostrar, des-minimizar y
                                    // reposicionar a una esquina segura siempre visible (80,80).
                                    if let Some(ui) = ui_weak.upgrade() {
                                        let _ = ui.show();
                                        ui.window().set_minimized(false);
                                        ui.window()
                                            .set_position(slint::LogicalPosition::new(80.0, 80.0));
                                    }
                                }
                                tray::TrayMsg::Exit => {
                                    ctrl.borrow().save_session();
                                    let _ = slint::quit_event_loop();
                                }
                            }
                        }
                    }
                    // Watcher de dispositivos (F5B): si cambiaron las unidades (USB), reubicar
                    // los paneles cuya carpeta desapareció, re-listarlos y refrescar la tira de
                    // discos. La misma lógica corre en `on_wake` (refresh en vivo).
                    apply_device_change();
                    // Asegurar que cada panel Files vigile su carpeta actual (barato si nada
                    // cambió). Arranca/re-arranca watchers tras navegar/agregar/cerrar paneles.
                    ctrl.borrow_mut().reconcile_watchers(waker.clone());
                    let files_done = ctrl.borrow_mut().pump_listings();
                    let tree_done = ctrl.borrow_mut().pump_tree();
                    let preview_busy = ctrl.borrow_mut().drive_preview(now);
                    let preview_ready = ctrl.borrow_mut().preview.poll().is_some();
                    let _ = preview_ready;
                    // Drenar el progreso de las operaciones de archivo (F3).
                    let ops_done = ctrl.borrow_mut().ops.pump_ops();
                    // Drenar el cálculo de tamaño de carpeta (F3 «calcular tamaño»).
                    let size_done = ctrl.borrow_mut().pump_sizes();
                    // Drenar la búsqueda recursiva en vuelo (Ctrl+F / lupa).
                    let search_done = ctrl.borrow_mut().pump_search();
                    // Drenar el listado profundo en vuelo (vista profunda / toggle).
                    let deep_changed = ctrl.borrow_mut().deep_poll();
                    // Watcher de carpeta (F5A): aplicar los cambios detectados a cada panel y
                    // marcar como nuevos los archivos recién aparecidos (para resaltarlos).
                    let batches = ctrl.borrow_mut().watchers.drain();
                    for (pane, events) in batches {
                        let nuevas = ctrl.borrow_mut().apply_watch_events(PaneId(pane), &events);
                        ctrl.borrow_mut().watchers.mark_fresh(pane, nuevas, now);
                    }
                    // Limpiar los resaltados vencidos y saber si queda alguno (para seguir
                    // pintando hasta que se apaguen).
                    let hl_secs = ctrl.borrow().highlight_secs();
                    ctrl.borrow_mut().watchers.prune(hl_secs, now);
                    // Reflejar la poda en el `highlighted` de cada panel: si la opción "archivos
                    // nuevos al final" está activa, las filas que ya no están frescas dejan de
                    // quedarse al final y vuelven a su orden normal.
                    ctrl.borrow_mut()
                        .sync_highlighted_from_watchers(hl_secs, now);
                    let fresh_pending = ctrl.borrow().watchers.any_fresh(hl_secs, now);
                    sync_rows();
                    // Persistir la sesión si cambió (agregar/cerrar/navegar paneles). Barato
                    // si no cambió. Antes de parar el timer, así el último cambio se guarda.
                    ctrl.borrow_mut().maybe_persist_session();
                    // El watcher corre en su propio hilo y despierta la UI con el waker; el
                    // timer puede dormir cuando no hay trabajo NI resaltados pendientes.
                    if files_done
                        && tree_done
                        && !preview_busy
                        && ops_done
                        && size_done
                        && search_done
                        && !deep_changed
                        && !fresh_pending
                    {
                        timer2.stop();
                    }
                },
            );
        })
    };
    start_timer();

    // `wake`: lo dispara un worker (watcher de carpeta/dispositivos) vía invoke_from_event_loop.
    // Corre en el hilo de UI. Además de re-arrancar el timer, aplica YA un posible cambio de
    // unidades: así un USB recién conectado refresca la tira de discos EN VIVO aunque el timer
    // estuviera dormido (el refresh no depende de que llegue un tick).
    {
        let start_timer = start_timer.clone();
        let apply_device_change = apply_device_change.clone();
        let ui_weak_wake = ui.as_weak();
        let logged_first_wake = std::rc::Rc::new(std::cell::Cell::new(false));
        ui.on_wake(move || {
            // Diagnóstico: en el PRIMER wake la ventana ya entró al event loop y debería tener
            // tamaño real. Si aquí sigue 0x0, el SO/compositor (típico en VM) no la dimensionó.
            // Se loguea una sola vez. Barato y no depende de símbolos de depuración.
            if !logged_first_wake.replace(true) {
                if let Some(ui) = ui_weak_wake.upgrade() {
                    let size = ui.window().size();
                    let scale = ui.window().scale_factor();
                    crate::logging::log_line(&format!(
                        "arranque: primer wake (ventana {}x{} @{:.2})",
                        size.width, size.height, scale
                    ));
                }
            }
            apply_device_change();
            start_timer();
        });
    }

    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let sync_layout = sync_layout.clone();
        ui.on_row_clicked(move |id, pos| {
            // El doble-clic se detecta en Rust (no en Slint): on_row_clicked devuelve true
            // si este clic completó un doble-clic, en cuyo caso navegó/abrió.
            let navigated = ctrl.borrow_mut().on_row_clicked(
                PaneId(id as u64),
                pos as usize,
                std::time::Instant::now(),
            );
            // Cambiar el foco/navegar puede disparar un preview o cambiar el layout.
            start_timer();
            if navigated {
                sync_layout();
            } else {
                sync_rows();
            }
        });
    }
    // Toggle de vista profunda (botón de la barra del panel Files).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_toggle_deep(move |id| {
            ctrl.borrow_mut().deep_toggle(PaneId(id as u64));
            start_timer();
            sync_rows();
        });
    }
    {
        // Doble clic NATIVO de Slint (cronometrado por el SO): camino primario para abrir
        // carpetas, robusto ante la latencia del hilo de UI bajo render por software (caso
        // VM). La detección por tiempo en Rust (en on_row_clicked) queda de respaldo; el
        // controlador evita la doble navegación con una marca temporal.
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_row_double_clicked(move |id, pos| {
            if ctrl
                .borrow_mut()
                .on_row_double_clicked_native(PaneId(id as u64), pos as usize)
            {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        // Arrastre OLE hacia afuera (Fase 5C): saca los archivos seleccionados del panel
        // hacia el Explorer/escritorio/otra app —o a otro panel de Naygo—. `start_drag` es
        // BLOQUEANTE: `DoDragDrop` corre su propio bucle modal de mensajes de Windows hasta
        // que el usuario suelta. Ese bucle modal RE-ENTRA los callbacks de Slint (el timer
        // que repinta, `sync_rows`, etc.) mientras dura el arrastre.
        //
        // Por eso NO podemos llamar `start_drag` aquí adentro: este callback lo invoca el
        // event loop de Slint dentro de un frame, y si `start_drag` entra a su bucle modal
        // con CUALQUIER `RefCell` del controlador prestado (o lo pide un re-entry), revienta
        // con «already borrowed». La causa raíz del crash al arrastrar entre paneles.
        //
        // Fix: capturar las rutas con un `borrow()` CORTO que termina YA, y diferir
        // `start_drag` al próximo turno del event loop con `invoke_from_event_loop`. Cuando
        // ese turno corre, garantizadamente no hay ningún borrow de `ctrl` vivo (el frame que
        // disparó este callback ya cerró), así el bucle modal de `DoDragDrop` no choca con
        // nadie. El cinturón de seguridad complementario está en el tick del timer, que ahora
        // usa `try_borrow_mut` y se salta el tick si el controlador está prestado.
        let ctrl = ctrl.clone();
        ui.on_row_drag_out(move |_id| {
            // Borrow corto: clonar las rutas seleccionadas y soltar el préstamo de inmediato.
            let paths = ctrl.borrow().selected_paths();
            if paths.is_empty() {
                return;
            }
            crate::logging::breadcrumb(&format!(
                "drag_out: iniciar arrastre ({} ítems)",
                paths.len()
            ));
            // Diferir el arrastre fuera de este frame. `paths` se mueve al closure; ya no hay
            // ningún borrow de `ctrl` en juego cuando `DoDragDrop` arranca su bucle modal.
            let _ = slint::invoke_from_event_loop(move || {
                let _outcome = naygo_platform::dnd::start_drag(&paths);
            });
        });
    }
    // Rubber-band (6F): selección por rectángulo arrastrando desde una fila no seleccionada.
    // Ctrl (estado del controlador) hace la selección aditiva.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_rubber_band(move |id, from, to| {
            let additive = ctrl.borrow().ctrl_down;
            ctrl.borrow_mut()
                .select_rect_range(PaneId(id as u64), from, to, additive);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_sort_by(move |_id, col| {
            ctrl.borrow_mut().on_sort_by(col.as_str());
            sync_rows();
        });
    }
    // Ordenar por columna dinámica (header de columnas) (6C).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_sort_by_kind(move |_id, kind| {
            ctrl.borrow_mut().sort_by_kind(kind);
            sync_rows();
        });
    }
    // Mostrar/ocultar una columna (menú "Columnas…") (6C).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_column_toggle(move |id, kind| {
            ctrl.borrow_mut().column_toggle(PaneId(id as u64), kind);
            sync_rows();
        });
    }
    // Reordenar columnas (arrastrar el header) (6C).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_column_move(move |id, from, to| {
            ctrl.borrow_mut().column_move(PaneId(id as u64), from, to);
            sync_rows();
        });
    }
    // Redimensionar una columna (arrastrar su borde) (6C).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_column_resize(move |id, kind, w| {
            ctrl.borrow_mut().column_resize(PaneId(id as u64), kind, w);
            sync_rows();
        });
    }
    // --- Menú/editor de columna (clic derecho en el header, F2) ---
    // Abrir el menú en (x,y) para la columna `kind` del panel `id`.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_column_context(move |id, kind, x, y| {
            ctrl.borrow_mut()
                .column_menu_open(PaneId(id as u64), kind, x, y);
            sync_rows();
        });
    }
    // Ordenar ascendente desde el menú.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_sort_asc(move || {
            ctrl.borrow_mut().column_menu_sort(true);
            sync_rows();
        });
    }
    // Ordenar descendente desde el menú.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_sort_desc(move || {
            ctrl.borrow_mut().column_menu_sort(false);
            sync_rows();
        });
    }
    // Pasar el menú a modo editor de filtro.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_open_filter(move || {
            ctrl.borrow_mut().column_menu_to_filter();
            sync_rows();
        });
    }
    // Quitar el filtro de la columna del menú.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_clear_filter(move || {
            ctrl.borrow_mut().column_menu_clear_filter();
            sync_rows();
        });
    }
    // Ocultar la columna del menú.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_hide(move || {
            ctrl.borrow_mut().column_menu_hide();
            sync_rows();
        });
    }
    // Mover la columna una posición a la izquierda.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_move_left(move || {
            ctrl.borrow_mut().column_menu_move(-1);
            sync_rows();
        });
    }
    // Mover la columna una posición a la derecha.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_move_right(move || {
            ctrl.borrow_mut().column_menu_move(1);
            sync_rows();
        });
    }
    // Editor de filtro: borrador de texto.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_set_text(move |t| {
            ctrl.borrow_mut().column_filter_set_text(t.as_str());
            sync_rows();
        });
    }
    // Editor de filtro: alternar sensibilidad a mayúsculas.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_toggle_case(move || {
            ctrl.borrow_mut().column_filter_toggle_case();
            sync_rows();
        });
    }
    // Editor de filtro: borrador del extremo mínimo (rango).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_set_min(move |t| {
            ctrl.borrow_mut().column_filter_set_range(false, t.as_str());
            sync_rows();
        });
    }
    // Editor de filtro: borrador del extremo máximo (rango).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_set_max(move |t| {
            ctrl.borrow_mut().column_filter_set_range(true, t.as_str());
            sync_rows();
        });
    }
    // Editor de filtro: marcar/desmarcar una extensión.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_toggle_ext(move |e| {
            ctrl.borrow_mut().column_filter_toggle_ext(e.as_str());
            sync_rows();
        });
    }
    // Editor de filtro: aplicar (cierra el menú; la vista se refiltra sola).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colfilter_apply(move || {
            ctrl.borrow_mut().column_filter_apply();
            sync_rows();
        });
    }
    // Cerrar el menú (clic fuera).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_colmenu_dismiss(move || {
            ctrl.borrow_mut().column_menu_close();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_activate(move |id| {
            ctrl.borrow_mut().set_active(PaneId(id as u64));
            start_timer();
            sync_rows();
        });
    }
    // Botones laterales del mouse: atrás/adelante en el panel donde se hizo clic.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_nav_back(move |id| {
            // El borrow del controlador se LIBERA antes de start_timer()/sync_rows(): esos
            // closures vuelven a tomar `ctrl`, y dejar `c` vivo causaba un doble-borrow del
            // RefCell (panic → cierre abrupto) cuando on_go_back devolvía false (sin historial).
            let moved = {
                let mut c = ctrl.borrow_mut();
                c.set_active(PaneId(id as u64));
                c.on_go_back()
            };
            if moved {
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_nav_forward(move |id| {
            // Igual que nav_back: liberar el borrow antes de start_timer()/sync_rows().
            let moved = {
                let mut c = ctrl.borrow_mut();
                c.set_active(PaneId(id as u64));
                c.on_go_forward()
            };
            if moved {
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let ui_weak = ui.as_weak();
        let palette_cmds = palette_cmds.clone();
        let palette_cmd_indices = palette_cmd_indices.clone();
        ui.on_key(move |text, c, s, a| {
            // Con la paleta de comandos abierta, su propio overlay (FocusScope del LineEdit) maneja
            // el teclado. Suspendemos el on_key global del panel para que las teclas (Enter/letras/
            // flechas) no disparen acciones por debajo. Mismo criterio que con un modal abierto.
            if let Some(ui) = ui_weak.upgrade() {
                if ui.get_palette_open() {
                    return;
                }
            }
            if ctrl.borrow_mut().on_key(text.as_str(), c, s, a) {
                start_timer();
            }
            // ¿La tecla pidió abrir la paleta de comandos (Ctrl+P)? Construir los comandos vigentes,
            // mostrar todos (query vacía) y abrir el overlay. El LineEdit toma el foco en `init`.
            if ctrl.borrow_mut().take_open_palette_request() {
                if let Some(ui) = ui_weak.upgrade() {
                    let cmds = ctrl.borrow().build_palette_commands();
                    let matches = naygo_core::palette::filter_and_rank(&cmds, "");
                    let (items, idxs) = palette_items_from_matches(&cmds, &matches);
                    *palette_cmds.borrow_mut() = cmds;
                    *palette_cmd_indices.borrow_mut() = idxs;
                    ui.set_palette_results(ModelRc::from(Rc::new(VecModel::from(items))));
                    ui.set_palette_query(SharedString::new());
                    ui.set_palette_selected(0);
                    ui.set_palette_open(true);
                    // El overlay de la paleta roba el foco al panel: su `key-released` no llega aquí.
                    // Limpiamos los modificadores para no dejar Ctrl pegado tras Ctrl+P al cerrar.
                    ctrl.borrow_mut().clear_modifiers();
                }
            }
            // Atajo "editar ruta" (Ctrl+L / F4): abrir el editor de la path-bar del panel pedido.
            if let Some(pane) = ctrl.borrow_mut().take_edit_path_request() {
                if let Some(ui) = ui_weak.upgrade() {
                    let path = ctrl.borrow().path_of(pane);
                    let sugg = ctrl.borrow().path_autocomplete(&path);
                    ui.set_edit_pane(pane.0 as i32);
                    ui.set_edit_text(path.into());
                    ui.set_edit_suggestions(ModelRc::from(Rc::new(VecModel::from(
                        sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
                    ))));
                }
            }
            // Rename inline (F2): abrir el editor en la celda Name de la fila pedida.
            if let Some((pane, pos, _stage)) = ctrl.borrow_mut().take_rename_request() {
                if let Some(ui) = ui_weak.upgrade() {
                    let name = ctrl.borrow().rename_name_at(pane, pos);
                    ui.set_rename_pane(pane.0 as i32);
                    ui.set_rename_pos(pos as i32);
                    ui.set_rename_text(name.into());
                }
            }
            start_timer();
            sync_layout();
        });
    }
    // Soltado de tecla: resetea `ctrl_down`/`shift_down` en el controlador con los modificadores
    // vigentes. Sin esto, tras un Ctrl+C el flag quedaba pegado en `true` y el siguiente doble-clic
    // en carpeta abría-en-otro-panel en vez de navegar. No refresca UI ni timers: sólo estado.
    {
        let ctrl = ctrl.clone();
        ui.on_key_release(move |c, s, a| {
            ctrl.borrow_mut().on_key_release(c, s, a);
        });
    }
    // Rename inline: confirmar (Enter). Renombra y cierra el editor. (6D)
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let ui_weak = ui.as_weak();
        ui.on_rename_commit(move |id, pos, name| {
            ctrl.borrow_mut()
                .rename_commit(PaneId(id as u64), pos as usize, name.as_str());
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_rename_pane(-1);
                ui.set_rename_pos(-1);
            }
            start_timer();
            sync_rows();
        });
    }
    // Rename inline: encadenar (↑/↓). Confirma el actual y reabre el editor en la fila vecina. (6D)
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let ui_weak = ui.as_weak();
        ui.on_rename_chain(move |id, pos, name, dir| {
            let pane = PaneId(id as u64);
            let next = ctrl
                .borrow_mut()
                .rename_chain(pane, pos as usize, name.as_str(), dir);
            if let Some(ui) = ui_weak.upgrade() {
                match next {
                    Some(p) => {
                        let nm = ctrl.borrow().rename_name_at(pane, p);
                        ui.set_rename_pane(pane.0 as i32);
                        ui.set_rename_pos(p as i32);
                        ui.set_rename_text(nm.into());
                    }
                    None => {
                        ui.set_rename_pane(-1);
                        ui.set_rename_pos(-1);
                    }
                }
            }
            start_timer();
            sync_rows();
        });
    }
    // Rename inline: cancelar (Esc). Cierra el editor sin renombrar. (6D)
    {
        let ui_weak = ui.as_weak();
        ui.on_rename_cancel(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_rename_pane(-1);
                ui.set_rename_pos(-1);
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_up(move || {
            if ctrl.borrow_mut().on_go_up() {
                start_timer();
            }
            sync_layout();
        });
    }
    // Navegación tipo navegador: Atrás / Adelante / Inicio. Cada uno relanza el listado si se
    // movió (start_timer) y repinta (sync_layout, que además actualiza can-go-back/forward).
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_back(move || {
            if ctrl.borrow_mut().on_go_back() {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_forward(move || {
            if ctrl.borrow_mut().on_go_forward() {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_home(move || {
            if ctrl.borrow_mut().on_go_home() {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_add_pane(move || {
            ctrl.borrow_mut().add_pane_split();
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_add_pane_of(move |purpose| {
            ctrl.borrow_mut().add_pane_of(int_to_purpose(purpose));
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_add_pane_dir(move |dir| {
            // 0=derecha 1=abajo 2=izquierda 3=arriba.
            let (split, first) = match dir {
                1 => (SplitDir::Vertical, false),
                2 => (SplitDir::Horizontal, true),
                3 => (SplitDir::Vertical, true),
                _ => (SplitDir::Horizontal, false),
            };
            ctrl.borrow_mut().add_pane_split_dir(split, first);
            start_timer();
            sync_layout();
        });
    }

    // --- Ventana de configuración (Fase 4) ---
    // Reconstruye el SettingsVm + filas de atajos desde ConfigCtrl y los vuelca a la UI.
    let refresh_config_vm: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        Rc::new(move || {
            let c = ctrl.borrow();
            let settings_vm = build_settings_vm(&c.config);
            // La AppWindow sigue usando `settings-vm` en la toolbar (icon-only, alto de fila),
            // así que se mantiene actualizado ahí además de en la ventana de configuración.
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_settings_vm(settings_vm.clone());
            }
            let Some(cfg) = cfg_weak.upgrade() else {
                return;
            };
            cfg.set_vm(settings_vm);
            // Poblar el campo de límite de recientes (no está en SettingsVm).
            cfg.set_recent_limit(c.config.settings.recent_limit as i32);
            // Auto-resaltado de código + footer (mostrar/plantilla/template/preview) + Home:
            // campos que no viven en SettingsVm; se vuelcan directo a las props de la ventana.
            cfg.set_auto_highlight_code(c.config.auto_highlight_code());
            cfg.set_footer_enabled(c.config.footer_enabled());
            cfg.set_footer_preset_index(c.config.footer_preset_index());
            cfg.set_footer_custom_template(c.config.footer_custom_template().into());
            cfg.set_footer_preview(c.config.footer_preview().into());
            cfg.set_home_dir(c.config.home_dir().into());
            let rows: Vec<ShortcutRowVm> = c
                .config
                .shortcut_list()
                .into_iter()
                .map(|(key, label, chord)| ShortcutRowVm {
                    action_key: key.into(),
                    label: label.into(),
                    chord_text: chord.into(),
                    conflict: SharedString::new(),
                })
                .collect();
            cfg.set_shortcuts(ModelRc::from(Rc::new(VecModel::from(rows))));
            // Reglas de previsualización (C3).
            let prev: Vec<PreviewRuleVm> = c
                .preview_rules()
                .into_iter()
                .map(|(ext, enabled, view_index, lang_index)| PreviewRuleVm {
                    ext: ext.into(),
                    enabled,
                    view_index,
                    lang_index,
                })
                .collect();
            cfg.set_preview_rules(ModelRc::from(Rc::new(VecModel::from(prev))));
            // Nombres legibles de los lenguajes de código para el combobox (orden de CodeLang::all()).
            let lang_names: Vec<SharedString> = naygo_core::preview::CodeLang::all()
                .iter()
                .map(|l| SharedString::from(code_lang_label(*l)))
                .collect();
            cfg.set_lang_names(ModelRc::from(Rc::new(VecModel::from(lang_names))));
            // Tarjetas de tema para la galería de selección (config → Apariencia).
            let active = c.config.settings.theme.clone();
            let col =
                |tc: naygo_core::theme::ThemeColor| slint::Color::from_rgb_u8(tc.r, tc.g, tc.b);
            let cards: Vec<ThemeCardVm> = c
                .config
                .themes
                .available()
                .iter()
                .map(|id| {
                    let t = c.config.themes.get(id);
                    ThemeCardVm {
                        id: id.as_str().into(),
                        name: t.name.clone().into(),
                        active: id.as_str() == active.as_str(),
                        is_builtin: naygo_core::theme::is_builtin_id(id.as_str()),
                        sw_panel: col(t.panel_bg),
                        sw_accent: col(t.accent),
                        sw_row: col(t.row_bg),
                        sw_text: col(t.text),
                        sw_highlight: col(t.highlight),
                    }
                })
                .collect();
            cfg.set_theme_cards(ModelRc::from(Rc::new(VecModel::from(cards))));
            // Estado del editor de temas (config → Apariencia). Cuando hay un tema en edición se
            // vuelcan su nombre/base y los 11 tokens (hex + r/g/b por canal, para inicializar el
            // color-picker, que no parsea hex).
            cfg.set_editing_active(c.config.is_editing_theme());
            cfg.set_editing_name(c.config.editing_name().into());
            cfg.set_editing_base_index(c.config.editing_base_index());
            let n = config_ctrl::THEME_TOKEN_COUNT;
            let mut hexes: Vec<SharedString> = Vec::with_capacity(n);
            let mut rs: Vec<i32> = Vec::with_capacity(n);
            let mut gs: Vec<i32> = Vec::with_capacity(n);
            let mut bs: Vec<i32> = Vec::with_capacity(n);
            if c.config.is_editing_theme() {
                for idx in 0..n {
                    hexes.push(c.config.editing_token_hex(idx).into());
                    let (r, g, b) = c.config.editing_token_rgb(idx);
                    rs.push(r as i32);
                    gs.push(g as i32);
                    bs.push(b as i32);
                }
            }
            cfg.set_editing_token_hex(ModelRc::from(Rc::new(VecModel::from(hexes))));
            cfg.set_editing_token_r(ModelRc::from(Rc::new(VecModel::from(rs))));
            cfg.set_editing_token_g(ModelRc::from(Rc::new(VecModel::from(gs))));
            cfg.set_editing_token_b(ModelRc::from(Rc::new(VecModel::from(bs))));
            cfg.set_config_dir(c.config.config_dir.to_string_lossy().to_string().into());
            cfg.set_app_version(env!("CARGO_PKG_VERSION").into());
            // Sección "Novedades": parsear el CHANGELOG embebido y volcar las notas de la
            // versión actual. Se setea una sola vez (no cambia en runtime).
            {
                let notes =
                    naygo_core::changelog::release_notes(CHANGELOG, env!("CARGO_PKG_VERSION"));
                let sections: Vec<NewsSection> = notes
                    .map(|n| {
                        n.sections
                            .into_iter()
                            .map(|s| NewsSection {
                                category: s.category.into(),
                                items: ModelRc::new(VecModel::from(
                                    s.items
                                        .into_iter()
                                        .map(SharedString::from)
                                        .collect::<Vec<_>>(),
                                )),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                cfg.set_release_notes(ModelRc::new(VecModel::from(sections)));
            }
        })
    };
    refresh_config_vm();
    // Vuelca los íconos de acción de la toolbar desde el IconCache a la AppWindow. Se llama al
    // arrancar y al cambiar el set de íconos. Decodifica una vez (cacheado): clonar es barato.
    let refresh_toolbar_icons: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        Rc::new(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            use naygo_core::icon_kind::{ActionIcon, IconKey};
            let mut c = ctrl.borrow_mut();
            let ic = |c: &mut workspace_ctrl::WorkspaceCtrl, a: ActionIcon| {
                c.icons.get(IconKey::Action(a))
            };
            ui.set_ic_up(ic(&mut c, ActionIcon::Up));
            ui.set_ic_panel(ic(&mut c, ActionIcon::Panel));
            ui.set_ic_swap(ic(&mut c, ActionIcon::SwapPanes));
            ui.set_ic_clone(ic(&mut c, ActionIcon::ClonePath));
            ui.set_ic_tabs(ic(&mut c, ActionIcon::Tabs));
            ui.set_ic_settings(ic(&mut c, ActionIcon::Settings));
            ui.set_ic_new_folder(ic(&mut c, ActionIcon::NewFolder));
            // (add-pane / layouts / terminal / refresh / eject: la toolbar volvió a íconos
            //  dibujados con Path —modelo flat—, así que ya no se vuelcan PNG para esos botones.)
        })
    };
    refresh_toolbar_icons();
    // Vuelca la tira de unidades de disco a la toolbar. Se llama al arrancar, al cambiar el set
    // de íconos (los íconos de disco cambian) y al cambiar los dispositivos (USB conectado/sacado).
    let refresh_drives: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        Rc::new(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let drives: Vec<NavRow> = ctrl
                .borrow_mut()
                .drive_strip()
                .into_iter()
                .map(to_nav_row)
                .collect();
            ui.set_drives(ModelRc::from(Rc::new(VecModel::from(drives))));
        })
    };
    refresh_drives();
    // Vuelca la lista de plantillas de disposición (built-in + usuario) al menú de Layouts (F4).
    // Se llama al arrancar y tras guardar/borrar una plantilla.
    let refresh_layouts: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        Rc::new(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let rows: Vec<LayoutRow> = ctrl
                .borrow()
                .layout_templates()
                .into_iter()
                .map(|(name, builtin)| LayoutRow {
                    name: SharedString::from(name.as_str()),
                    builtin,
                })
                .collect();
            ui.set_layout_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
        })
    };
    refresh_layouts();
    // Aplicar una plantilla: reconstruye el workspace y relanza el contenido de cada panel.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let refresh_layouts = refresh_layouts.clone();
        ui.on_apply_layout(move |name| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            ctrl.borrow_mut().apply_template(name.as_str(), now);
            start_timer();
            sync_layout();
            refresh_layouts();
        });
    }
    // Guardar la disposición actual como plantilla de usuario.
    {
        let ctrl = ctrl.clone();
        let refresh_layouts = refresh_layouts.clone();
        ui.on_save_layout(move |name| {
            ctrl.borrow_mut().save_current_template(name.as_str());
            refresh_layouts();
        });
    }
    // Borrar una plantilla de usuario.
    {
        let ctrl = ctrl.clone();
        let refresh_layouts = refresh_layouts.clone();
        ui.on_delete_layout(move |name| {
            ctrl.borrow_mut().delete_template(name.as_str());
            refresh_layouts();
        });
    }
    // --- Renombrado por lotes (F5): setters del spec (cada uno re-renderiza el preview) ---
    macro_rules! batch_setter_str {
        ($on:ident, $method:ident) => {{
            let ctrl = ctrl.clone();
            let sync_rows = sync_rows.clone();
            ui.$on(move |v: slint::SharedString| {
                ctrl.borrow_mut().$method(v.as_str());
                sync_rows();
            });
        }};
    }
    macro_rules! batch_setter_bool {
        ($on:ident, $method:ident) => {{
            let ctrl = ctrl.clone();
            let sync_rows = sync_rows.clone();
            ui.$on(move |v: bool| {
                ctrl.borrow_mut().$method(v);
                sync_rows();
            });
        }};
    }
    batch_setter_str!(on_batch_set_template, batch_set_template);
    batch_setter_str!(on_batch_set_find, batch_set_find);
    batch_setter_str!(on_batch_set_replace, batch_set_replace);
    batch_setter_str!(on_batch_set_counter_start, batch_set_counter_start);
    batch_setter_str!(on_batch_set_counter_step, batch_set_counter_step);
    batch_setter_bool!(on_batch_set_regex, batch_set_regex);
    batch_setter_bool!(on_batch_set_include_ext, batch_set_include_ext);
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_batch_set_case(move |i| {
            ctrl.borrow_mut().batch_set_case(i);
            sync_rows();
        });
    }
    // Aplicar: lanza la op de batch-rename (deshacible) y refresca el panel.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_batch_apply(move || {
            ctrl.borrow_mut().batch_apply();
            start_timer();
            sync_layout();
        });
    }
    // Cerrar sin aplicar.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_batch_close(move || {
            ctrl.borrow_mut().batch_close();
            sync_rows();
        });
    }
    // Cerrar la ayuda (Esc/clic fuera/✕).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_help_close(move || {
            ctrl.borrow_mut().help_close();
            sync_rows();
        });
    }
    // Paleta de comandos (Ctrl+P): se escribió en el campo → re-filtrar con la query nueva sobre
    // los comandos vigentes. Reconstruye los resultados y reinicia la selección al primero.
    {
        let ui_weak = ui.as_weak();
        let palette_cmds = palette_cmds.clone();
        let palette_cmd_indices = palette_cmd_indices.clone();
        ui.on_palette_query_changed(move |q| {
            if let Some(ui) = ui_weak.upgrade() {
                let cmds = palette_cmds.borrow();
                let matches = naygo_core::palette::filter_and_rank(&cmds, q.as_str());
                let (items, idxs) = palette_items_from_matches(&cmds, &matches);
                *palette_cmd_indices.borrow_mut() = idxs;
                ui.set_palette_results(ModelRc::from(Rc::new(VecModel::from(items))));
                ui.set_palette_selected(0);
            }
        });
    }
    // Paleta: cerrar sin ejecutar (Esc / clic fuera).
    {
        let ui_weak = ui.as_weak();
        ui.on_palette_dismiss(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_palette_open(false);
            }
        });
    }
    // Paleta: ejecutar el resultado de índice `result_idx`. La fila lleva el índice del COMANDO
    // (tabla paralela `palette_cmd_indices`). Ejecuta, cierra la paleta, y consume los pedidos que
    // el comando pudo dejar: re-aplicar tema (a ambas ventanas) y/o abrir configuración. Refresca.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        let sync_layout = sync_layout.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let palette_cmds = palette_cmds.clone();
        let palette_cmd_indices = palette_cmd_indices.clone();
        ui.on_palette_run(move |result_idx| {
            let cmd_idx = palette_cmd_indices
                .borrow()
                .get(result_idx.max(0) as usize)
                .copied();
            let Some(cmd_idx) = cmd_idx else {
                // Sin resultados (índice fuera de rango): solo cerrar.
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_palette_open(false);
                }
                return;
            };
            {
                let cmds = palette_cmds.borrow();
                if ctrl.borrow_mut().execute_palette_command(&cmds, cmd_idx) {
                    start_timer();
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_palette_open(false);
            }
            // Tema elegido en la paleta: re-pintar ambas ventanas con el tema activo.
            if let Some(_id) = ctrl.borrow_mut().take_palette_theme_request() {
                let c = ctrl.borrow();
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, c.config.active_theme());
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, c.config.active_theme());
                }
            }
            // "Abrir configuración" desde la paleta: reusa el handler del engranaje de la toolbar.
            if ctrl.borrow_mut().take_open_config_request() {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_open_config();
                }
            }
            sync_layout();
            sync_rows();
        });
    }
    // Acción cuyo atajo se está capturando (la setea cfg-shortcut-capture; la lee cfg-capture-key).
    let capturing_action: Rc<RefCell<Option<naygo_core::keymap::Action>>> =
        Rc::new(RefCell::new(None));

    // Toggles/combos/text que solo persisten (no requieren refrescar la vista de paneles).
    // Todos los handlers se registran ahora en la ventana de configuración (`cfg_win`), no en la
    // AppWindow: los callbacks viven en ConfigWindow SIN el prefijo `cfg-`.
    macro_rules! cfg_setter {
        ($on:ident, $arg:ty, $method:ident) => {{
            let ctrl = ctrl.clone();
            let refresh = refresh_config_vm.clone();
            cfg_win.$on(move |v: $arg| {
                ctrl.borrow_mut().config.$method(v);
                refresh();
            });
        }};
    }
    // ops_mode no usa el macro: además de persistir, hay que aplicar el modo al motor de ops
    // (cola/paralelo) en caliente vía sync_ops_mode.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_ops_mode(move |v| {
            {
                let mut c = ctrl.borrow_mut();
                c.config.set_ops_mode(v);
                c.sync_ops_mode();
            }
            refresh();
        });
    }
    cfg_setter!(on_set_confirm_trash, bool, set_confirm_trash);
    cfg_setter!(on_set_show_op_summary, bool, set_show_op_summary);
    cfg_setter!(on_set_show_parent, bool, set_show_parent);
    cfg_setter!(on_set_icon_only, bool, set_icon_only);
    cfg_setter!(on_set_bar_position, i32, set_bar_position);
    cfg_setter!(on_set_size_no_subdirs, bool, set_size_no_subdirs);
    cfg_setter!(on_set_autostart, bool, set_autostart);
    cfg_setter!(on_set_date_format, i32, set_date_format);
    cfg_setter!(on_set_size_format, i32, set_size_format);
    cfg_setter!(on_set_row_density, i32, set_row_density);
    // Avanzado (F3c).
    cfg_setter!(on_set_ops_display, i32, set_ops_display);
    cfg_setter!(on_set_paste_image_fmt, i32, set_paste_image_fmt);
    cfg_setter!(on_set_low_power_mode, i32, set_low_power_mode);
    cfg_setter!(on_set_new_items_at_end, bool, set_new_items_at_end);
    cfg_setter!(on_set_tray_enabled, bool, set_tray_enabled);
    cfg_setter!(on_set_close_to_tray, bool, set_close_to_tray);
    cfg_setter!(on_set_paste_confirm, bool, set_paste_confirm);
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_paste_text_name(move |v| {
            ctrl.borrow_mut().config.set_paste_text_name(v.to_string());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_paste_text_ext(move |v| {
            ctrl.borrow_mut().config.set_paste_text_ext(v.to_string());
            refresh();
        });
    }
    // Límite de carpetas recientes (Avanzado): persiste + trunca la lista al nuevo tope.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_recent_limit_changed(move |v| {
            ctrl.borrow_mut().set_recent_limit(v as usize);
            refresh();
        });
    }
    // Auto-resaltado de código (Previsualización): persiste el toggle.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_auto_highlight_code(move |v| {
            ctrl.borrow_mut().config.set_auto_highlight_code(v);
            refresh();
        });
    }
    // Pie de panel (Avanzado): mostrar/ocultar.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_footer_enabled(move |v| {
            ctrl.borrow_mut().config.set_footer_enabled(v);
            refresh();
        });
    }
    // Pie de panel: plantilla por índice (0..3 fijas, 4=Personalizada).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_footer_preset_index(move |idx| {
            ctrl.borrow_mut().config.set_footer_preset_index(idx);
            refresh();
        });
    }
    // Pie de panel: template personalizado. Refresca para recalcular la vista previa en vivo.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_footer_custom_template(move |t| {
            ctrl.borrow_mut()
                .config
                .set_footer_custom_template(t.to_string());
            refresh();
        });
    }
    // Carpeta de inicio (Home, Avanzado): edición directa del campo.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_home_dir(move |dir| {
            ctrl.borrow_mut().config.set_home_dir(dir.to_string());
            refresh();
        });
    }
    // Carpeta de inicio: botón Examinar → diálogo de carpeta nativo; aplica la ruta elegida.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_browse_home(move || {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                ctrl.borrow_mut()
                    .config
                    .set_home_dir(path.to_string_lossy().to_string());
                refresh();
            }
        });
    }
    // Cambio de idioma en caliente: persiste + re-vuelca todos los textos a Tr. Se aplica a
    // AMBAS ventanas (principal y config), porque cada una tiene su propia copia del global Tr.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_set_language(move |code| {
            let lang = naygo_core::i18n::LangId::new(&code);
            ctrl.borrow_mut().config.set_language(lang);
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                i18n_keys::apply(&ui, &c.config);
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                i18n_keys::apply(&cfg, &c.config);
            }
            drop(c);
            refresh();
        });
    }
    // Cambio de tema en caliente: persiste + re-vuelca los colores a Theme. Se aplica a AMBAS
    // ventanas (principal y config), porque cada una tiene su propia copia del global Theme.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_set_theme(move |id| {
            ctrl.borrow_mut()
                .config
                .set_theme(naygo_core::theme::ThemeId::new(&id));
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                theme_apply::apply(&ui, c.config.active_theme());
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                theme_apply::apply(&cfg, c.config.active_theme());
            }
            drop(c);
            refresh();
        });
    }
    // === Editor de temas (config → Apariencia) ===
    // "Personalizar" (builtin) / "Editar" (de usuario): abre el editor y aplica el tema en edición
    // como preview en vivo en AMBAS ventanas. El refresh vuelca el estado del editor a la UI.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_customize(move |id| {
            ctrl.borrow_mut().config.duplicate_theme(&id);
            let c = ctrl.borrow();
            if let Some(t) = c.config.editing_theme() {
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, t);
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, t);
                }
            }
            drop(c);
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_edit(move |id| {
            ctrl.borrow_mut().config.edit_user_theme(&id);
            let c = ctrl.borrow();
            if let Some(t) = c.config.editing_theme() {
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, t);
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, t);
                }
            }
            drop(c);
            refresh();
        });
    }
    // Eliminar un tema de usuario: borra el .json, recarga catálogo y, si era el activo, cae al
    // default. Re-aplica el tema activo resultante a ambas ventanas.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_delete(move |id| {
            ctrl.borrow_mut().config.delete_user_theme(&id);
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                theme_apply::apply(&ui, c.config.active_theme());
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                theme_apply::apply(&cfg, c.config.active_theme());
            }
            drop(c);
            refresh();
        });
    }
    // Nombre del tema en edición (no re-aplica preview: el nombre no es un color).
    {
        let ctrl = ctrl.clone();
        cfg_win.on_theme_set_name(move |name| {
            ctrl.borrow_mut().config.set_editing_name(name.to_string());
        });
    }
    // Base oscuro/claro del tema en edición.
    {
        let ctrl = ctrl.clone();
        cfg_win.on_theme_set_base(move |idx| {
            ctrl.borrow_mut().config.set_editing_base(idx);
        });
    }
    // Cambiar un token de color → preview en vivo en ambas ventanas + refresh (para repintar las
    // muestras/hex del editor).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_set_token(move |idx, hex| {
            ctrl.borrow_mut()
                .config
                .set_token_color(idx.max(0) as usize, &hex);
            let c = ctrl.borrow();
            if let Some(t) = c.config.editing_theme() {
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, t);
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, t);
                }
            }
            drop(c);
            refresh();
        });
    }
    // Guardar el tema en edición: escribe el .json, lo deja activo y re-aplica el tema activo.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_save(move || {
            ctrl.borrow_mut().config.save_editing_theme();
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                theme_apply::apply(&ui, c.config.active_theme());
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                theme_apply::apply(&cfg, c.config.active_theme());
            }
            drop(c);
            refresh();
        });
    }
    // Restaurar de fábrica: resetea los 11 tokens del tema en edición al builtin del que se
    // duplicó y re-aplica el preview.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_restore(move || {
            ctrl.borrow_mut().config.restore_factory_editing();
            let c = ctrl.borrow();
            if let Some(t) = c.config.editing_theme() {
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, t);
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, t);
                }
            }
            drop(c);
            refresh();
        });
    }
    // Cancelar: descarta el tema en edición y re-aplica el tema que estaba activo antes (revierte
    // el preview).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_cancel(move || {
            // Cancelar solo descarta el tema en edición: el tema ACTIVO nunca cambió (el editor solo
            // hacía preview en vivo, no persistía). No volver a llamar `set_theme` (evita una
            // escritura redundante de settings.json en el hilo de UI); el `apply(active_theme())` de
            // abajo revierte el preview al tema activo previo.
            ctrl.borrow_mut().config.cancel_editing();
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                theme_apply::apply(&ui, c.config.active_theme());
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                theme_apply::apply(&cfg, c.config.active_theme());
            }
            drop(c);
            refresh();
        });
    }
    // Cambio de set de íconos en caliente: persiste + apunta el IconCache al set nuevo. Las
    // filas repintan en el próximo tick (sync_rows consulta el cache, que decodifica el set
    // nuevo on-demand). El refresh re-snapshotea el VM de config para reflejar la selección.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let refresh_icons = refresh_toolbar_icons.clone();
        let refresh_drives = refresh_drives.clone();
        cfg_win.on_set_icon_set(move |id| {
            {
                let mut c = ctrl.borrow_mut();
                c.config.set_icon_set(id.to_string());
                // Tomar el id ya coaccionado por el catálogo (un id inválido cayó a "flat").
                let active = c.config.settings.icon_set.clone();
                c.icons.set_active(active);
            }
            // Repintar los íconos de la toolbar y de la tira de discos con el set nuevo.
            refresh_icons();
            refresh_drives();
            refresh();
        });
    }
    // Editor de atajos: capturar la acción a reasignar.
    {
        let capturing = capturing_action.clone();
        cfg_win.on_shortcut_capture(move |key| {
            *capturing.borrow_mut() = config_ctrl::ConfigCtrl::action_from_key(&key);
        });
    }
    // Captura de la combinación: si hay acción en captura y el chord es válido, reasigna.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let capturing = capturing_action.clone();
        cfg_win.on_capture_key(move |text, c, s, a| {
            let action = match capturing.borrow_mut().take() {
                Some(act) => act,
                None => return,
            };
            // Esc cancela la captura sin reasignar (la UI ya salió del modo captura).
            if text == keys::escape_char().to_string() {
                return;
            }
            if let Some(chord) = keys::chord_from(&text, c, s, a) {
                ctrl.borrow_mut().config.rebind(action, chord);
                refresh();
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_shortcut_reset(move |key| {
            if let Some(action) = config_ctrl::ConfigCtrl::action_from_key(&key) {
                ctrl.borrow_mut().config.reset_shortcut(action);
                refresh();
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_shortcuts_reset_all(move || {
            ctrl.borrow_mut().config.reset_all_shortcuts();
            refresh();
        });
    }
    // Import/Export de packs (.zip) — Fase 4E. Selector de archivo nativo (rfd); el resultado
    // se informa con un MessageDialog (errores) sin bloquear el resto de la UI.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_export_language(move || {
            let c = ctrl.borrow();
            let code = c.config.settings.language.as_str().to_string();
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Pack Naygo (.zip)", &["zip"])
                .set_file_name(format!("naygo-idioma-{code}.zip"))
                .save_file()
            {
                report(
                    &ui_weak,
                    packs::export_lang(&c.config.config_dir, &code, &path),
                );
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_export_theme(move || {
            let c = ctrl.borrow();
            let id = c.config.settings.theme.as_str().to_string();
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Pack Naygo (.zip)", &["zip"])
                .set_file_name(format!("naygo-tema-{id}.zip"))
                .save_file()
            {
                report(
                    &ui_weak,
                    packs::export_theme(&c.config.config_dir, &id, &path),
                );
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_export_config(move || {
            let c = ctrl.borrow();
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Pack Naygo (.zip)", &["zip"])
                .set_file_name("naygo-config.zip")
                .save_file()
            {
                report(&ui_weak, packs::export_config(&c.config.config_dir, &path));
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_import_pack(move || {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Pack Naygo (.zip)", &["zip"])
                .pick_file()
            else {
                return;
            };
            // Importar y, según el tipo, recargar el catálogo correspondiente.
            let config_dir = ctrl.borrow().config.config_dir.clone();
            match packs::import_zip(&config_dir, &path) {
                Ok(kind) => {
                    {
                        let mut cb = ctrl.borrow_mut();
                        let lang = cb.config.settings.language.clone();
                        let theme = cb.config.settings.theme.clone();
                        match kind {
                            packs::ImportKind::Lang(_) => {
                                cb.config.i18n = naygo_core::i18n::I18n::load(&config_dir, &lang);
                            }
                            packs::ImportKind::Theme(_) => {
                                cb.config.themes =
                                    naygo_core::theme::ThemeCatalog::load(&config_dir, &theme);
                            }
                            packs::ImportKind::Config => {
                                let fresh = config_ctrl::ConfigCtrl::new(config_dir.clone());
                                cb.config = fresh;
                            }
                        }
                    }
                    // Reaplicar textos y colores en AMBAS ventanas por si cambió el catálogo.
                    let c = ctrl.borrow();
                    if let Some(ui) = ui_weak.upgrade() {
                        i18n_keys::apply(&ui, &c.config);
                        theme_apply::apply(&ui, c.config.active_theme());
                    }
                    if let Some(cfg) = cfg_weak.upgrade() {
                        i18n_keys::apply(&cfg, &c.config);
                        theme_apply::apply(&cfg, c.config.active_theme());
                    }
                    drop(c);
                    refresh();
                }
                Err(e) => report(&ui_weak, Err::<(), String>(e)),
            }
        });
    }
    // Avanzado: factory reset. Restablece TODOS los ajustes y reaplica idioma/tema (cambian).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_factory_reset(move || {
            ctrl.borrow_mut().config.factory_reset();
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                i18n_keys::apply(&ui, &c.config);
                theme_apply::apply(&ui, c.config.active_theme());
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                i18n_keys::apply(&cfg, &c.config);
                theme_apply::apply(&cfg, c.config.active_theme());
            }
            drop(c);
            refresh();
        });
    }
    // C4: guardar la tabla del panel activo como plantilla por defecto.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_save_default_table(move || {
            ctrl.borrow_mut().save_default_table_from_active();
            refresh();
        });
    }
    // C4: limpiar la plantilla de tabla por defecto.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_clear_default_table(move || {
            ctrl.borrow_mut().clear_default_table();
            refresh();
        });
    }
    // C3: reglas de previsualización (toggle / tratar-como / quitar / agregar).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_preview_toggle(move |ext| {
            ctrl.borrow_mut().preview_rule_toggle(ext.as_str());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_preview_set_view_mode(move |ext, idx| {
            ctrl.borrow_mut()
                .preview_rule_set_view_mode(ext.as_str(), idx);
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_preview_set_view_lang(move |ext, idx| {
            ctrl.borrow_mut()
                .preview_rule_set_view_lang(ext.as_str(), idx);
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_preview_remove(move |ext| {
            ctrl.borrow_mut().preview_rule_remove(ext.as_str());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_preview_add(move |ext| {
            ctrl.borrow_mut().preview_rule_add(ext.as_str());
            refresh();
        });
    }
    // Acerca de: abrir el repositorio en el navegador por defecto.
    {
        cfg_win.on_open_repo(move || {
            let _ = naygo_platform::open::open_default(std::path::Path::new(
                "https://github.com/nicolasgroth/explorador_archivos_naygo",
            ));
        });
    }
    // Acerca de (easter egg): primeros `n` caracteres del mensaje (substring que Slint no ofrece).
    // Lee el global Tr de la PROPIA ventana de config (donde se muestra el easter egg).
    {
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_egg_prefix(move |n| {
            let Some(cfg) = cfg_weak.upgrade() else {
                return SharedString::new();
            };
            let msg = cfg.global::<Tr>().get_about_egg_message();
            let take = n.max(0) as usize;
            SharedString::from(msg.chars().take(take).collect::<String>())
        });
    }
    // Cerrar la ventana de config (botón "cerrar" interno): la oculta (no destruye la instancia).
    {
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_close(move || {
            if let Some(cfg) = cfg_weak.upgrade() {
                let _ = cfg.hide();
            }
        });
    }
    // Abrir la ventana de config desde el engranaje de la toolbar: refresca el VM (para que abra
    // poblada) y la muestra.
    {
        let refresh = refresh_config_vm.clone();
        let cfg_weak = cfg_win.as_weak();
        let ctrl = ctrl.clone();
        ui.on_open_config(move || {
            crate::logging::breadcrumb("abrir configuración");
            refresh();
            // La ventana de config es otra ventana y roba el foco: el `key-released` no vuelve al
            // panel. Limpiamos los modificadores para no dejar Ctrl/Shift pegados al volver.
            ctrl.borrow_mut().clear_modifiers();
            if let Some(cfg) = cfg_weak.upgrade() {
                let _ = cfg.show();
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_tree_toggle(move |id, path| {
            ctrl.borrow_mut()
                .tree_toggle(PaneId(id as u64), std::path::PathBuf::from(path.as_str()));
            start_timer();
            sync_layout();
        });
    }
    // Navegación por teclado del árbol (↑↓←→/Enter): mueve el cursor, expande/colapsa o navega.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_tree_key(move |id, key| {
            ctrl.borrow_mut().tree_key(PaneId(id as u64), key.as_str());
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_tree_navigate(move |path| {
            if ctrl
                .borrow_mut()
                .navigate_active_to(std::path::PathBuf::from(path.as_str()))
            {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_nav_navigate(move |path| {
            if ctrl
                .borrow_mut()
                .navigate_active_to(std::path::PathBuf::from(path.as_str()))
            {
                start_timer();
            }
            sync_layout();
        });
    }
    // Ícono de historial de carpetas (reloj) en la toolbar: arma las filas y abre el menú flotante.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_history_open(move |x| {
            let rows: Vec<NavRow> = {
                let mut c = ctrl.borrow_mut();
                c.recents.remove_missing();
                c.recent_rows().into_iter().map(to_nav_row).collect()
            };
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_history_rows(ModelRc::new(VecModel::from(rows)));
                ui.set_history_menu_x(x);
                // Cerrar los demás menús para que nunca haya dos abiertos a la vez (discos +
                // historiales ▾ de Atrás/Adelante).
                ui.set_drive_menu_path("".into());
                ui.set_back_history_menu_open(false);
                ui.set_fwd_history_menu_open(false);
                ui.set_history_menu_open(true);
            }
        });
    }
    // Menú del "ojo" (visibilidad): abrir anclado al botón. Cierra los demás menús flotantes.
    {
        let ui_weak = ui.as_weak();
        ui.on_open_view_menu(move |x| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_view_menu_x(x);
                // No tener dos menús abiertos a la vez.
                ui.set_history_menu_open(false);
                ui.set_back_history_menu_open(false);
                ui.set_fwd_history_menu_open(false);
                ui.set_drive_menu_path("".into());
                ui.set_view_menu_open(true);
            }
        });
    }
    // Toggles del menú del "ojo": alternan el flag (persiste), re-arman los árboles (las
    // subcarpetas se re-listan filtradas) y refrescan la vista. Los paneles se refiltran solos en
    // el próximo `sync_rows`; el menú queda abierto para alternar varios de una.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_toggle_show_hidden(move || {
            {
                let mut c = ctrl.borrow_mut();
                let v = c.config.settings.show_hidden;
                c.config.set_show_hidden(!v);
                c.refresh_trees_visibility();
            }
            start_timer();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_toggle_show_system(move || {
            {
                let mut c = ctrl.borrow_mut();
                let v = c.config.settings.show_system;
                c.config.set_show_system(!v);
                c.refresh_trees_visibility();
            }
            start_timer();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_toggle_hide_dotfiles(move || {
            {
                let mut c = ctrl.borrow_mut();
                let v = c.config.settings.hide_dotfiles;
                c.config.set_hide_dotfiles(!v);
                c.refresh_trees_visibility();
            }
            start_timer();
            sync_rows();
        });
    }
    // Elegir una carpeta del menú de historial: navega el panel activo.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_history_pick(move |path| {
            if ctrl
                .borrow_mut()
                .navigate_active_to(std::path::PathBuf::from(path.as_str()))
            {
                start_timer();
            }
            sync_layout();
            sync_rows();
        });
    }
    // ▾ del botón Atrás: arma las carpetas hacia atrás del panel activo y abre el menú anclado.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_open_back_history(move |x| {
            let items: Vec<HistoryItemVm> = ctrl
                .borrow()
                .back_history_entries()
                .iter()
                .map(|p| path_to_history_item(p))
                .collect();
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_history_items(ModelRc::new(VecModel::from(items)));
                ui.set_back_history_menu_x(x);
                // No tener dos menús abiertos a la vez: cerrar adelante + recientes + discos.
                ui.set_fwd_history_menu_open(false);
                ui.set_history_menu_open(false);
                ui.set_drive_menu_path("".into());
                ui.set_back_history_menu_open(true);
            }
        });
    }
    // ▾ del botón Adelante: arma las carpetas hacia adelante del panel activo y abre el menú.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_open_forward_history(move |x| {
            let items: Vec<HistoryItemVm> = ctrl
                .borrow()
                .forward_history_entries()
                .iter()
                .map(|p| path_to_history_item(p))
                .collect();
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_history_items(ModelRc::new(VecModel::from(items)));
                ui.set_fwd_history_menu_x(x);
                ui.set_back_history_menu_open(false);
                ui.set_history_menu_open(false);
                ui.set_drive_menu_path("".into());
                ui.set_fwd_history_menu_open(true);
            }
        });
    }
    // Elegir una entrada del menú ▾ de Atrás: el controlador traduce el índice del menú al de la
    // pila y salta sin re-apilar. Refresca el listado y los menús se cierran solos en la UI.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_back_history(move |menu_index| {
            if ctrl
                .borrow_mut()
                .go_back_history(menu_index.max(0) as usize)
            {
                start_timer();
            }
            sync_layout();
            sync_rows();
        });
    }
    // Elegir una entrada del menú ▾ de Adelante: ídem con la rama de adelante.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_forward_history(move |menu_index| {
            if ctrl
                .borrow_mut()
                .go_forward_history(menu_index.max(0) as usize)
            {
                start_timer();
            }
            sync_layout();
            sync_rows();
        });
    }
    // Clic en una unidad de la tira de discos de la toolbar: navega el panel activo a su raíz.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_nav_drive(move |path| {
            if ctrl
                .borrow_mut()
                .navigate_active_to(std::path::PathBuf::from(path.as_str()))
            {
                start_timer();
            }
            sync_layout();
        });
    }
    // Expulsar de forma segura una unidad extraíble (botón ⏏ o clic derecho sobre la unidad).
    // Es una llamada de un solo disparo (desmontar+expulsar, sin sondeo); en la práctica retorna
    // de inmediato. El resultado se anuncia con un toast localizado. NUNCA fuerza: si el volumen
    // está en uso, el toast lo dice y la unidad queda intacta. En éxito refrescamos la tira (la
    // unidad desaparece) — además el device_watch la detectará igual.
    // Path pendiente de expulsar mientras el modal de confirmación está abierto.
    let pending_eject: std::rc::Rc<std::cell::RefCell<Option<String>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    {
        let ui_weak = ui.as_weak();
        let pending_eject = pending_eject.clone();
        ui.on_eject_drive(move |path| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            // Confirmación con el modal temático de Naygo (evita sacar una unidad por un clic
            // accidental). La expulsión real ocurre en on_message_confirm; aquí solo guardamos el
            // path pendiente y abrimos el modal.
            let tr = ui.global::<Tr>();
            let body = tr
                .get_drive_eject_confirm()
                .replace("{drive}", path.as_str());
            *pending_eject.borrow_mut() = Some(path.to_string());
            ui.set_message(MessageVm {
                kind: 1,
                level: 1, // warning
                title: tr.get_drive_eject_confirm_title(),
                body: body.into(),
                confirm_label: tr.get_drive_eject(),
                cancel_label: tr.get_dlg_cancel(),
                danger: false,
            });
        });
    }
    // Confirmación del modal temático: ejecuta la expulsión pendiente (kind 1).
    {
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        let refresh_drives = refresh_drives.clone();
        let pending_eject = pending_eject.clone();
        ui.on_message_confirm(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let kind = ui.get_message().kind;
            // Cerrar el modal.
            ui.set_message(MessageVm::default());
            // kind 1 = confirmación de expulsar.
            if kind == 1 {
                if let Some(path) = pending_eject.borrow_mut().take() {
                    let outcome = ctrl
                        .borrow()
                        .eject_drive(std::path::PathBuf::from(path.as_str()));
                    let tr = ui.global::<Tr>();
                    let msg = match outcome {
                        workspace_ctrl::EjectOutcome::Ok => {
                            refresh_drives();
                            tr.get_drive_eject_ok()
                        }
                        workspace_ctrl::EjectOutcome::InUse => tr.get_drive_eject_in_use(),
                        workspace_ctrl::EjectOutcome::Failed(_) => tr.get_drive_eject_failed(),
                    };
                    ui.invoke_show_toast(msg);
                }
            }
        });
    }
    // Cancelación del modal temático: cierra y descarta el path pendiente.
    {
        let ui_weak = ui.as_weak();
        let pending_eject = pending_eject.clone();
        ui.on_message_cancel(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            ui.set_message(MessageVm::default());
            pending_eject.borrow_mut().take();
        });
    }
    // Refrescar unidades a mano (botón ⟳): re-escanea y reconstruye la tira. Útil para unidades
    // de red, que no disparan WM_DEVICECHANGE.
    {
        let refresh_drives = refresh_drives.clone();
        ui.on_refresh_drives(move || refresh_drives());
    }
    // --- Barra de ruta (breadcrumbs + edición + autocompletado) ---
    {
        // Clic en un breadcrumb: navegar ese panel a la ruta del segmento.
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_path_segment_clicked(move |id, path| {
            if ctrl
                .borrow_mut()
                .navigate_pane_to(PaneId(id as u64), std::path::PathBuf::from(path.as_str()))
            {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        // Entrar a modo edición: cargar la ruta actual del panel y sus candidatos.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_path_edit_start(move |id| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let path = ctrl.borrow().path_of(PaneId(id as u64));
            let sugg = ctrl.borrow().path_autocomplete(&path);
            ui.set_edit_pane(id);
            ui.set_edit_text(path.into());
            ui.set_edit_suggestions(ModelRc::from(Rc::new(VecModel::from(
                sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
            ))));
        });
    }
    {
        // El texto del editor cambió: recalcular el autocompletado.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_path_edit_changed(move |_id, text| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let sugg = ctrl.borrow().path_autocomplete(text.as_str());
            ui.set_edit_text(text);
            ui.set_edit_suggestions(ModelRc::from(Rc::new(VecModel::from(
                sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
            ))));
        });
    }
    {
        // Enter en el editor: navegar a la ruta tecleada (si existe como carpeta) y salir.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_path_edit_commit(move |id, text| {
            let dir = std::path::PathBuf::from(text.as_str());
            if dir.is_dir() && ctrl.borrow_mut().navigate_pane_to(PaneId(id as u64), dir) {
                start_timer();
            }
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_edit_pane(-1);
            }
            sync_layout();
        });
    }
    {
        // Esc: salir de edición sin navegar.
        let ui_weak = ui.as_weak();
        ui.on_path_edit_cancel(move |_id| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_edit_pane(-1);
            }
        });
    }
    // ★ favorito de la path-bar: anclar/quitar la carpeta de ese panel.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_toggle(move |id| {
            ctrl.borrow_mut().toggle_favorite_dir(PaneId(id as u64));
            sync_rows();
        });
    }
    // 📋 copiar la ruta de la carpeta de ese panel al portapapeles. Además del toast (Slint) y
    // el ✓ del ícono, se anuncia en la barra de estado (se restaura en la próxima interacción).
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_copy_path(move |id| {
            ctrl.borrow().copy_pane_path(PaneId(id as u64));
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_status(ui.global::<Tr>().get_pathbar_copied());
            }
        });
    }
    {
        // Clic en un candidato: completar el último segmento del editor y seguir editando.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_path_suggestion_clicked(move |_id, name| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let buffer = ui.get_edit_text().to_string();
            let (parent, _) = naygo_core::path_segments::split_edit_buffer(&buffer);
            // Completar: padre + nombre elegido + separador (listo para seguir bajando).
            let completed = format!("{parent}{name}\\");
            let sugg = ctrl.borrow().path_autocomplete(&completed);
            ui.set_edit_text(completed.into());
            ui.set_edit_suggestions(ModelRc::from(Rc::new(VecModel::from(
                sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
            ))));
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_remove(move |path| {
            ctrl.borrow_mut()
                .remove_favorite(std::path::Path::new(path.as_str()));
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_pin_current(move || {
            ctrl.borrow_mut().toggle_favorite_active();
            sync_rows();
        });
    }
    // --- Árbol de favoritos editable (panel + menú ▾ del toolbar) ---
    {
        // Expandir/colapsar un grupo del árbol por su ruta de nombres (estado de UI, no persiste).
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_toggle_expand(move |name_path| {
            ctrl.borrow_mut().fav_toggle_expand(name_path.as_str());
            sync_rows();
        });
    }
    {
        // Crear un grupo nuevo: (parent-group-id serializado, nombre). Persiste y refresca.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_new_group(move |parent_gid, name| {
            ctrl.borrow_mut()
                .fav_new_group(parent_gid.as_str(), name.as_str());
            sync_rows();
        });
    }
    {
        // Renombrar un grupo: (group-id serializado, nombre nuevo). Persiste y refresca.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_rename_group(move |gid, name| {
            ctrl.borrow_mut()
                .fav_rename_group(gid.as_str(), name.as_str());
            sync_rows();
        });
    }
    {
        // Eliminar un nodo: (is-group, group-id, path). Persiste y refresca.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_delete_node(move |is_group, gid, path| {
            ctrl.borrow_mut()
                .fav_delete_node(is_group, gid.as_str(), path.as_str());
            sync_rows();
        });
    }
    {
        // Mover un nodo: (is-group, src-group-id, src-path, dest-group-id; "" = raíz). Persiste.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_move_node(move |is_group, src_gid, src_path, dest_gid| {
            ctrl.borrow_mut().fav_move_node(
                is_group,
                src_gid.as_str(),
                src_path.as_str(),
                dest_gid.as_str(),
            );
            sync_rows();
        });
    }
    {
        // "Mover a…": al abrir el submenú, recalcular los destinos VÁLIDOS para el nodo elegido
        // (Rust excluye el propio grupo y sus descendientes) y volcarlos en `fav-move-targets`.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_fav_build_move_targets(move |is_group, src_gid| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let targets: Vec<PathSeg> = ctrl
                .borrow()
                .fav_move_targets(is_group, src_gid.as_str())
                .into_iter()
                .map(|(label, gid)| PathSeg {
                    label: SharedString::from(label),
                    path: SharedString::from(gid),
                })
                .collect();
            ui.set_fav_move_targets(ModelRc::from(Rc::new(VecModel::from(targets))));
        });
    }
    {
        // Botón "abrir con el programa del sistema" del panel de vista previa: ShellExecute
        // sobre la ruta del archivo previsualizado (la app no edita; abre con el editor del SO).
        ui.on_preview_open(move |path| {
            if !path.is_empty() {
                let _ = naygo_platform::open::open_default(std::path::Path::new(path.as_str()));
            }
        });
    }
    {
        // Botón "Deshacer" del panel Historial: deshace la entrada por id.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_undo_entry(move |id| {
            if ctrl.borrow_mut().undo_entry(id as u64) {
                start_timer();
            }
            sync_rows();
        });
    }
    // --- Diálogos modales y panel de progreso de operaciones (F3) ---
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_delete_confirm(move || {
            // delete_confirm devuelve true si arrancó una op larga (eliminación permanente por el
            // motor). La papelera es atómica y no devuelve true, así que el panel no aparece para
            // ella. Si arrancó algo, asegurar el panel de Operaciones (auto-aparecer).
            let started = ctrl.borrow_mut().ops.delete_confirm();
            if started {
                ctrl.borrow_mut().ensure_ops_pane();
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_delete_cancel(move || {
            ctrl.borrow_mut().ops.dialog_cancel();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_conflict_decide(move |action, apply_all| {
            use naygo_core::ops::ConflictAction;
            let act = match action {
                0 => ConflictAction::Overwrite,
                2 => ConflictAction::Rename,
                _ => ConflictAction::Skip,
            };
            // El id estable de la op en conflicto lo guarda el pending_dialog.
            let op_id = {
                let c = ctrl.borrow();
                if let Some(ops_ctrl::OpDialog::Conflict { op_id, .. }) = &c.ops.pending_dialog {
                    Some(*op_id)
                } else {
                    None
                }
            };
            if let Some(op_id) = op_id {
                ctrl.borrow_mut()
                    .ops
                    .resolve_conflict(op_id, act, apply_all);
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_conflict_cancel(move || {
            // Cancelar TODA la operación desde el modal de conflicto (botón "Cancelar todo",
            // Esc o clic fuera). Toma el id estable de la op en conflicto del pending_dialog,
            // igual que `on_conflict_decide`, y cancela su token sin enviar una decisión: el
            // worker está esperando con `recv_timeout` y, al ver el token cancelado, aborta.
            let op_id = {
                let c = ctrl.borrow();
                if let Some(ops_ctrl::OpDialog::Conflict { op_id, .. }) = &c.ops.pending_dialog {
                    Some(*op_id)
                } else {
                    None
                }
            };
            if let Some(op_id) = op_id {
                ctrl.borrow_mut().ops.cancel_conflict(op_id);
                // Mantener el timer vivo para que `pump_ops` drene el `Cancelled`/`Skipped` del
                // worker y cierre la op (progreso → historial), igual que un cancel normal.
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_folder_conflict_decide(move |decision, apply_all| {
            // Conflicto de CARPETA (P3): aplicar la decisión (0=fusionar 1=reemplazar 2=saltar) a
            // la op detenida en el conflicto. El id estable lo guarda el pending_dialog.
            let op_id = {
                let c = ctrl.borrow();
                if let Some(ops_ctrl::OpDialog::FolderConflict { op_id, .. }) =
                    &c.ops.pending_dialog
                {
                    Some(*op_id)
                } else {
                    None
                }
            };
            if let Some(op_id) = op_id {
                ctrl.borrow_mut()
                    .ops
                    .resolve_folder_conflict(op_id, decision, apply_all);
                // Reactivar el timer: si quedan más carpetas, `pump_ops` reabre el modal; si no,
                // arranca el motor (o el escaneo de la cola) y hay que drenarlo.
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_folder_conflict_cancel(move || {
            // Cancelar TODA la operación desde el conflicto de carpeta (botón, Esc o velo).
            let op_id = {
                let c = ctrl.borrow();
                if let Some(ops_ctrl::OpDialog::FolderConflict { op_id, .. }) =
                    &c.ops.pending_dialog
                {
                    Some(*op_id)
                } else {
                    None
                }
            };
            if let Some(op_id) = op_id {
                ctrl.borrow_mut().ops.cancel_folder_conflict(op_id);
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        ui.on_name_changed(move |v| {
            ctrl.borrow_mut().ops.name_changed(v.to_string());
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_name_confirm(move || {
            let confirmed = ctrl.borrow_mut().ops.name_confirm();
            if confirmed {
                // Re-listar el panel activo tras crear/pegar: el pegado escribe el archivo
                // directo (sin pasar por el motor de ops), y el listado debe refrescarse para
                // que aparezca. Además `refresh_active` re-lista con `enter()`, que LIMPIA la
                // selección/foco; sin esto, tras pegar quedaba un foco de vista "stale" que
                // bloqueaba los clics del mouse hasta reseleccionar.
                ctrl.borrow_mut().refresh_active();
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_name_cancel(move || {
            ctrl.borrow_mut().ops.dialog_cancel();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_paste_confirm(move || {
            // El pegado de texto/imagen se cablea con el journal/encode; por ahora cierra.
            ctrl.borrow_mut().ops.dialog_cancel();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_paste_cancel(move || {
            ctrl.borrow_mut().ops.dialog_cancel();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_op_cancel(move |id| {
            // `id` es el id ESTABLE de la op (lo emite `OpRowData.index`), no su posición.
            ctrl.borrow_mut().ops.cancel_op(id);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_op_pause(move |id| {
            ctrl.borrow_mut().ops.pause_op(id);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_op_resume(move |id| {
            ctrl.borrow_mut().ops.resume_op(id);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_op_skip(move |id| {
            // Saltar el archivo en curso aún no está soportado por el motor (no-op).
            ctrl.borrow_mut().ops.skip_op(id);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_resume_decide(move |id, action| {
            let id = id.to_string();
            let mut c = ctrl.borrow_mut();
            if action == 0 {
                if c.ops.resume(&id) {
                    // Retomar arranca una transferencia larga: mostrar el panel de Operaciones.
                    c.ensure_ops_pane();
                    drop(c);
                    start_timer();
                }
            } else {
                c.ops.discard(&id);
            }
            sync_rows();
        });
    }
    // --- Menú contextual (clic derecho): acciones propias + nativo ---
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_row_context(move |_id, _pos, x, y| {
            ctrl.borrow_mut().open_context_menu(x, y);
            sync_rows();
        });
    }
    // Clic derecho en la zona vacía del panel → menú contextual de la carpeta (modo carpeta).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_empty_context(move |id, x, y| {
            ctrl.borrow_mut()
                .open_folder_context_menu(PaneId(id as u64), x, y);
            sync_rows();
        });
    }
    // Modo carpeta: abrir el Explorador de Windows en la carpeta.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_explorer(move || {
            ctrl.borrow_mut().ctx_open_explorer();
            sync_rows();
        });
    }
    // Modo carpeta: abrir el modal "nueva(s) carpeta(s)".
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_new_folder(move || {
            ctrl.borrow_mut().ctx_new_folder();
            sync_rows();
        });
    }
    // Modal "nueva(s) carpeta(s)": editar texto / crear / cancelar.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_new_folder_set_text(move |t| {
            ctrl.borrow_mut().new_folder_set_text(&t);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_new_folder_create(move || {
            ctrl.borrow_mut().new_folder_apply();
            start_timer();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_new_folder_close(move || {
            ctrl.borrow_mut().new_folder_close();
            sync_rows();
        });
    }
    // "Carpeta no encontrada" IN-PLACE por panel: reintentar / subir / elegir / cerrar. Cada
    // handler recibe el pane-id del panel que muestra el aviso.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_missing_retry(move |id| {
            ctrl.borrow_mut().missing_folder_retry(PaneId(id as u64));
            start_timer();
            // Reconstruir el PaneVm con el estado actual de `missing`: el aviso se refresca al
            // instante (antes solo se actualizaba al redimensionar, porque el listado async ya
            // había apagado el timer cuando corría el siguiente sync_rows).
            sync_rows();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_missing_ancestor(move |id| {
            ctrl.borrow_mut()
                .missing_folder_go_ancestor(PaneId(id as u64));
            start_timer();
            sync_rows();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_missing_choose(move |id| {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                ctrl.borrow_mut()
                    .missing_folder_choose(PaneId(id as u64), path);
                start_timer();
            }
            sync_rows();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_missing_close_pane(move |id| {
            ctrl.borrow_mut()
                .missing_folder_close_pane(PaneId(id as u64));
            start_timer();
            sync_rows();
            sync_layout();
        });
    }
    // Búsqueda recursiva (Ctrl+F / lupa): lanzar / cerrar / detener / abrir resultado / alternar.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_search_run(move |q| {
            ctrl.borrow_mut().start_search(q.to_string());
            start_timer();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_search_close(move || {
            ctrl.borrow_mut().close_search();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_search_cancel(move || {
            ctrl.borrow_mut().cancel_search();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_search_open(move |i| {
            ctrl.borrow_mut().open_search_hit(i.max(0) as usize);
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_search_toggle(move || {
            // La lupa alterna: si hay panel abierto, lo cierra; si no, lo abre vacío (sin lanzar
            // todavía — el usuario escribe y pulsa Enter/Buscar). Abrir = sembrar un job inactivo
            // mostrando el panel; lo modelamos arrancando una búsqueda con query vacía no sirve
            // (no abre), así que abrimos con un marcador: reusamos open_empty_search.
            let open = ctrl.borrow().search_open();
            if open {
                ctrl.borrow_mut().close_search();
            } else {
                ctrl.borrow_mut().open_empty_search();
            }
            sync_rows();
        });
    }
    // Toolbar: nueva carpeta en el panel activo (abre el modal).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_new_folder_toolbar(move || {
            ctrl.borrow_mut().new_folder_open_active();
            sync_rows();
        });
    }
    // Toolbar: combo de terminales en el panel activo (0=PS,1=CMD,2=WT,3=WSL).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_toolbar_terminal(move |term| {
            ctrl.borrow_mut().terminal_active(term);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_dismiss(move || {
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_open(move || {
            ctrl.borrow_mut().ctx_open();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_open_with(move || {
            ctrl.borrow_mut().ctx_open_with();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_copy(move || {
            ctrl.borrow_mut().op_copy();
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_cut(move || {
            ctrl.borrow_mut().op_cut();
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_ctx_paste(move || {
            if ctrl.borrow_mut().op_paste() {
                start_timer();
            }
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let ui_weak = ui.as_weak();
        ui.on_ctx_rename(move || {
            ctrl.borrow_mut().op_rename();
            ctrl.borrow_mut().close_context_menu();
            // Abrir el editor inline en la fila pedida (igual que el camino de F2).
            if let Some((pane, pos, _stage)) = ctrl.borrow_mut().take_rename_request() {
                if let Some(ui) = ui_weak.upgrade() {
                    let name = ctrl.borrow().rename_name_at(pane, pos);
                    ui.set_rename_pane(pane.0 as i32);
                    ui.set_rename_pos(pos as i32);
                    ui.set_rename_text(name.into());
                }
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_delete(move || {
            ctrl.borrow_mut().op_delete(false);
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_copy_path(move || {
            ctrl.borrow_mut().ctx_copy_path();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_copy_names(move || {
            ctrl.borrow_mut().ctx_copy_names();
            sync_rows();
        });
    }
    // Abrir terminal en la carpeta: 0=PowerShell, 1=CMD, 2=Windows Terminal.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_terminal_ps(move || {
            ctrl.borrow_mut().ctx_open_terminal(0);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_terminal_cmd(move || {
            ctrl.borrow_mut().ctx_open_terminal(1);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_terminal_wt(move || {
            ctrl.borrow_mut().ctx_open_terminal(2);
            sync_rows();
        });
    }
    {
        // "Más opciones de Windows…": invoca el menú nativo del Shell con el HWND de winit.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_native(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let (targets, x, y) = {
                let c = ctrl.borrow();
                match &c.context_menu {
                    Some(cm) => (cm.targets.clone(), cm.x, cm.y),
                    None => return,
                }
            };
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
            if let Some(hwnd) = naygo_hwnd(&ui) {
                // Coords de pantalla = posición de la ventana + posición del clic en la ventana.
                let pos = ui.window().position();
                let sx = pos.x + x as i32;
                let sy = pos.y + y as i32;
                let _ =
                    naygo_platform::context_menu::show_native_context_menu(hwnd, &targets, sx, sy);
            }
        });
    }
    // --- Acciones multi-panel (swap / clonar) + selector de destino ---
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let area_of = area_of.clone();
        ui.on_swap_panes(move || {
            let area = area_of();
            let acted = {
                let mut c = ctrl.borrow_mut();
                let Some(origin) = c.active_id() else {
                    return;
                };
                c.request_action(workspace_ctrl::PaneAction::Swap, origin, area)
            };
            if acted {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let area_of = area_of.clone();
        ui.on_clone_pane(move || {
            let area = area_of();
            let acted = {
                let mut c = ctrl.borrow_mut();
                let Some(origin) = c.active_id() else {
                    return;
                };
                c.request_action(workspace_ctrl::PaneAction::Clone, origin, area)
            };
            if acted {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let area_of = area_of.clone();
        ui.on_stack_pane(move || {
            let area = area_of();
            {
                let mut c = ctrl.borrow_mut();
                let Some(origin) = c.active_id() else {
                    return;
                };
                c.request_action(workspace_ctrl::PaneAction::Stack, origin, area);
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_tab_select(move |id| {
            ctrl.borrow_mut().set_active_tab(PaneId(id as u64));
            // Cambiar de pestaña puede disparar el preview del nuevo foco.
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_tab_close(move |id| {
            ctrl.borrow_mut().close_tab(PaneId(id as u64));
            sync_layout();
        });
    }
    {
        // Durante el arrastre: resaltar la zona de drop bajo el puntero.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let area_of = area_of.clone();
        ui.on_pane_drag_move(move |id, x, y| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let area = area_of();
            let preview = ctrl.borrow().drop_preview(PaneId(id as u64), x, y, area);
            match preview {
                Some((r, is_tab)) => {
                    ui.set_drop_x(r.x);
                    ui.set_drop_y(r.y);
                    ui.set_drop_w(r.w);
                    ui.set_drop_h(r.h);
                    ui.set_drop_is_tab(is_tab);
                }
                None => {
                    ui.set_drop_w(0.0);
                    ui.set_drop_h(0.0);
                }
            }
        });
    }
    {
        // Al soltar: recomponer el layout y limpiar el resaltado.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let sync_layout = sync_layout.clone();
        let area_of = area_of.clone();
        ui.on_pane_drag_drop(move |id, x, y| {
            let area = area_of();
            ctrl.borrow_mut()
                .perform_drop(PaneId(id as u64), x, y, area);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_drop_w(0.0);
                ui.set_drop_h(0.0);
            }
            sync_layout();
        });
    }
    // Cerrar/quitar un panel (X del título o clic derecho).
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_close_pane(move |id| {
            ctrl.borrow_mut().close_pane(PaneId(id as u64));
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_pick_resolve(move |n| {
            if ctrl.borrow_mut().pick_resolve(n as usize) {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_pick_cancel(move || {
            ctrl.borrow_mut().pick_cancel();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let area_of = area_of.clone();
        let ui_weak = ui.as_weak();
        // ARRASTRE EN VIVO: NO reflowar el layout (eso no repinta bajo el render por software y
        // se ve distorsionado). Solo calcular dónde quedaría el borde y pintar la barra-fantasma
        // (un Rectangle por escalares, que sí repinta al instante). `px`/`py` = posición ABSOLUTA
        // del puntero en coords de contenido.
        ui.on_split_drag(move |index, px, py| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let area = area_of();
            let c = ctrl.borrow();
            let handles = c.split_handles(area);
            if let Some(h) = handles.get(index as usize) {
                if let Some((_f, bar)) = c.fraction_at(&h.path.clone(), area, px, py) {
                    ui.set_splitpreview_x(bar.x);
                    ui.set_splitpreview_y(bar.y);
                    ui.set_splitpreview_w(bar.w);
                    ui.set_splitpreview_h(bar.h);
                }
            }
        });
    }
    // COMMIT (al soltar): aplicar la fraction de verdad, reflowar UNA vez, y limpiar la fantasma.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let area_of = area_of.clone();
        let ui_weak = ui.as_weak();
        ui.on_split_commit(move |index, px, py| {
            let area = area_of();
            {
                let mut c = ctrl.borrow_mut();
                let handles = c.split_handles(area);
                if let Some(h) = handles.get(index as usize) {
                    let path = h.path.clone();
                    if let Some((f, _bar)) = c.fraction_at(&path, area, px, py) {
                        c.set_fraction(&path, f);
                    }
                }
            }
            // Ocultar la barra-fantasma (w/h = 0).
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_splitpreview_w(0.0);
                ui.set_splitpreview_h(0.0);
            }
            sync_layout();
        });
    }
    {
        let sync_layout = sync_layout.clone();
        ui.on_content_resized(move || sync_layout());
    }

    // Al cerrar la ventana (Fase 5E, arregla la deuda de F4): persistir la sesión y luego SALIR
    // DE VERDAD (quit_event_loop), salvo que el usuario haya pedido "cerrar a bandeja" y el tray
    // esté activo, en cuyo caso se oculta a la bandeja y la app sigue viva a propósito.
    {
        let ctrl = ctrl.clone();
        ui.window().on_close_requested(move || {
            ctrl.borrow().save_session();
            let close_to_tray = ctrl.borrow().config.settings.close_to_tray;
            if tray::should_quit_on_close(close_to_tray, tray_active) {
                let _ = slint::quit_event_loop();
            }
            // En ambos casos HideWindow: al salir, el loop ya está marcado para terminar; al ir
            // a bandeja, la ventana se oculta y el proceso sigue.
            slint::CloseRequestResponse::HideWindow
        });
    }

    // Al arrancar: si hay operaciones interrumpidas (journal), ofrecer retomarlas.
    ctrl.borrow_mut().ops.scan_resume();
    sync_layout();
    // Línea de entorno para el log (versión/OS/ventana). Se fija aquí, tras sync_layout(),
    // cuando la ventana ya tiene tamaño real. Una sola vez.
    {
        let size = ui.window().size();
        let scale = ui.window().scale_factor();
        crate::logging::set_env_info((size.width, size.height), scale, &os_version_string());
    }
    // Hito de arranque: entrando al event loop. El tamaño aquí suele ser 0x0 (la ventana aún no
    // fue dimensionada por el SO); el tamaño REAL se registra en el primer `on_wake` (ya dentro
    // del loop). Sirve para acotar dónde cae un panic de arranque en máquinas problemáticas.
    {
        let size = ui.window().size();
        crate::logging::log_line(&format!(
            "arranque: entrando al event loop (ventana pre-loop {}x{})",
            size.width, size.height
        ));
    }
    ui.run()
}

fn int_to_purpose(p: i32) -> PanePurpose {
    match p {
        1 => PanePurpose::Tree,
        2 => PanePurpose::Inspector,
        3 => PanePurpose::History,
        4 => PanePurpose::Favorites,
        5 => PanePurpose::Preview,
        6 => PanePurpose::Operations,
        _ => PanePurpose::Files,
    }
}

/// Muestra un modal de error (temático) si el resultado de un import/export falló;
/// silencioso si OK.
fn report<T>(ui: &slint::Weak<AppWindow>, r: Result<T, String>) {
    if let Err(e) = r {
        if let Some(ui) = ui.upgrade() {
            let tr = ui.global::<Tr>();
            ui.set_message(MessageVm {
                kind: 2,
                level: 2, // error
                title: "Naygo".into(),
                body: e.into(),
                confirm_label: tr.get_dlg_accept(),
                cancel_label: Default::default(),
                danger: false,
            });
        }
    }
}

/// Construye el `SettingsVm` (snapshot para la ventana de config) desde el ConfigCtrl.
fn build_settings_vm(c: &config_ctrl::ConfigCtrl) -> SettingsVm {
    use naygo_core::config::{BarPosition, OpsMode};
    let s = &c.settings;
    let languages: Vec<SharedString> = c
        .i18n
        .available()
        .iter()
        .map(|l| SharedString::from(l.as_str()))
        .collect();
    let themes: Vec<SharedString> = c
        .themes
        .available()
        .iter()
        .map(|t| SharedString::from(t.as_str()))
        .collect();
    let icon_sets: Vec<SharedString> = naygo_core::icon_set::IconSetCatalog::load(&c.config_dir)
        .available()
        .iter()
        .map(|s| SharedString::from(s.id.as_str()))
        .collect();
    SettingsVm {
        bar_position: if s.bar_position == BarPosition::Side {
            1
        } else {
            0
        },
        icon_only: s.icon_only,
        show_parent: s.show_parent_entry,
        ops_mode: if s.ops_mode == OpsMode::Parallel {
            1
        } else {
            0
        },
        confirm_trash: s.confirm_trash,
        show_op_summary: s.show_op_summary,
        size_no_subdirs: s.size_no_subdirs,
        autostart: s.autostart,
        date_format: match s.date_format {
            naygo_core::format::DateFormat::IsoMinute => 0,
            naygo_core::format::DateFormat::IsoDate => 1,
            naygo_core::format::DateFormat::DmyMinute => 2,
            naygo_core::format::DateFormat::DmyDate => 3,
        },
        size_format: match s.size_format {
            naygo_core::format::SizeFormat::Auto => 0,
            naygo_core::format::SizeFormat::Bytes => 1,
            naygo_core::format::SizeFormat::Kb => 2,
            naygo_core::format::SizeFormat::Mb => 3,
        },
        row_density: match s.row_density {
            naygo_core::config::RowDensity::Compact => 0,
            naygo_core::config::RowDensity::Comfortable => 1,
        },
        row_h: s.row_density.row_height(),
        ops_display: match s.ops_display {
            naygo_core::config::OpsDisplay::Panel => 0,
            naygo_core::config::OpsDisplay::Modal => 1,
            naygo_core::config::OpsDisplay::AlwaysVisible => 2,
        },
        paste_image_fmt: match s.paste_image_fmt {
            naygo_core::clipboard::ImageFmt::Png => 0,
            naygo_core::clipboard::ImageFmt::Jpg => 1,
        },
        tray_enabled: s.tray_enabled,
        close_to_tray: s.close_to_tray,
        new_items_at_end: s.new_items_at_end,
        low_power_mode: match s.low_power_mode {
            naygo_core::config::LowPowerMode::Auto => 0,
            naygo_core::config::LowPowerMode::Always => 1,
            naygo_core::config::LowPowerMode::Never => 2,
        },
        default_table_on: s.default_table.is_some(),
        paste_confirm: s.paste_confirm,
        paste_text_name: s.paste_text_name.clone().into(),
        paste_text_ext: s.paste_text_ext.clone().into(),
        language: s.language.as_str().into(),
        theme: s.theme.as_str().into(),
        icon_set: s.icon_set.as_str().into(),
        languages: ModelRc::from(Rc::new(VecModel::from(languages))),
        themes: ModelRc::from(Rc::new(VecModel::from(themes))),
        icon_sets: ModelRc::from(Rc::new(VecModel::from(icon_sets))),
    }
}

/// El HWND de la ventana de Naygo (backend winit), para el menú contextual del Shell.
/// `None` si no se puede obtener (otro backend) — entonces se oculta "Más opciones de
/// Windows…". Usa raw-window-handle vía el feature `raw-window-handle-06` de slint.
fn naygo_hwnd(ui: &AppWindow) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = ui.window().window_handle();
    match handle.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(isize::from(h.hwnd)),
        _ => None,
    }
}

/// El `PreviewVm` actual a partir del último resultado guardado en el controlador. El
/// resultado vivo se entrega por `poll()` en el timer y se cachea en el ctrl; aquí lo
/// reconstruimos para pintarlo. (Mantener la última vista evita parpadeo entre ticks.)
/// Nombre legible de un lenguaje de código para el combobox de Configuración (no es texto i18n:
/// son nombres propios de lenguajes, iguales en todos los idiomas).
fn code_lang_label(lang: naygo_core::preview::CodeLang) -> &'static str {
    use naygo_core::preview::CodeLang;
    match lang {
        CodeLang::Xml => "XML",
        CodeLang::Json => "JSON",
        CodeLang::Html => "HTML",
        CodeLang::Css => "CSS",
        CodeLang::JavaScript => "JavaScript",
        CodeLang::C => "C",
        CodeLang::Cpp => "C++",
        CodeLang::Java => "Java",
        CodeLang::Python => "Python",
        CodeLang::Rust => "Rust",
        CodeLang::Sql => "SQL",
        CodeLang::Bash => "Bash",
        CodeLang::Markdown => "Markdown",
        CodeLang::Yaml => "YAML",
        CodeLang::Toml => "TOML",
        CodeLang::Ini => "INI",
    }
}

fn current_preview_vm(c: &WorkspaceCtrl) -> PreviewVm {
    // Ruta del archivo cargado (para "abrir con el programa del sistema"); "" si ninguno.
    let path: SharedString = c
        .preview
        .loaded
        .as_ref()
        .map(|p| SharedString::from(p.to_string_lossy().as_ref()))
        .unwrap_or_default();
    match c.preview.last_view() {
        Some(preview::ViewCache::Text {
            text,
            truncated,
            highlighted,
        }) => {
            // Si el worker resaltó el texto, mapeamos cada línea/segmento al modelo de la UI; el
            // color (u8,u8,u8) de core se vuelve `slint::Color::from_rgb_u8`.
            let (is_hl, hl_lines): (bool, Vec<HlLineVm>) = match highlighted {
                Some(lines) => (
                    true,
                    lines
                        .iter()
                        .map(|l| {
                            let spans: Vec<HlSpanVm> = l
                                .spans
                                .iter()
                                .map(|s| HlSpanVm {
                                    text: SharedString::from(s.text.as_str()),
                                    color: slint::Color::from_rgb_u8(
                                        s.color.0, s.color.1, s.color.2,
                                    ),
                                })
                                .collect();
                            HlLineVm {
                                spans: ModelRc::from(Rc::new(VecModel::from(spans))),
                            }
                        })
                        .collect(),
                ),
                None => (false, Vec::new()),
            };
            PreviewVm {
                mode: 1,
                text: SharedString::from(text.as_str()),
                truncated: *truncated,
                image: slint::Image::default(),
                message: SharedString::new(),
                highlighted: is_hl,
                hl_lines: ModelRc::from(Rc::new(VecModel::from(hl_lines))),
                path,
            }
        }
        Some(preview::ViewCache::Image {
            rgba,
            width,
            height,
        }) => {
            let buf = SharedPixelBuffer::clone_from_slice(rgba, *width, *height);
            PreviewVm {
                mode: 2,
                text: SharedString::new(),
                truncated: false,
                image: slint::Image::from_rgba8(buf),
                message: SharedString::new(),
                highlighted: false,
                hl_lines: ModelRc::default(),
                path,
            }
        }
        Some(preview::ViewCache::Message(m)) => PreviewVm {
            mode: 3,
            text: SharedString::new(),
            truncated: false,
            image: slint::Image::default(),
            message: SharedString::from(m.as_str()),
            highlighted: false,
            hl_lines: ModelRc::default(),
            path,
        },
        None => PreviewVm {
            mode: 0,
            text: SharedString::new(),
            truncated: false,
            image: slint::Image::default(),
            message: SharedString::new(),
            highlighted: false,
            hl_lines: ModelRc::default(),
            path: SharedString::new(),
        },
    }
}

fn to_row_data(r: bridge::PlainRow) -> RowData {
    let cells: Vec<SharedString> = r
        .cells
        .iter()
        .map(|c| SharedString::from(c.as_str()))
        .collect();
    RowData {
        name: SharedString::from(r.name.as_str()),
        cells: ModelRc::from(Rc::new(VecModel::from(cells))),
        is_dir: r.is_dir,
        selected: r.selected,
        focused: r.focused,
        cut: r.cut,
        highlight: r.highlight,
        icon: r.icon,
        depth: r.depth as i32,
    }
}

fn to_column_vm(c: bridge::ColumnInfo) -> ColumnVm {
    ColumnVm {
        kind: c.kind,
        label: SharedString::from(c.label.as_str()),
        width: c.width,
        align_right: c.align_right,
        sort_dir: c.sort_dir,
        has_filter: c.has_filter,
    }
}

fn to_column_toggle_vm(c: bridge::ColumnToggle) -> ColumnToggleVm {
    ColumnToggleVm {
        kind: c.kind,
        label: SharedString::from(c.label.as_str()),
        visible: c.visible,
        fixed: c.fixed,
    }
}

fn to_op_dialog_vm(d: ops_ctrl::OpDialogVmData) -> OpDialogVm {
    OpDialogVm {
        kind: d.kind,
        del_count: d.del_count,
        del_permanent: d.del_permanent,
        conflict_name: SharedString::from(d.conflict_name.as_str()),
        name_title: SharedString::from(d.name_title.as_str()),
        name_value: SharedString::from(d.name_value.as_str()),
        name_valid: d.name_valid,
        paste_name: SharedString::from(d.paste_name.as_str()),
        paste_is_image: d.paste_is_image,
        folder_name: SharedString::from(d.folder_name.as_str()),
        folder_more: d.folder_more,
    }
}

fn to_op_row_vm(r: ops_ctrl::OpRowData) -> OpRowVm {
    OpRowVm {
        index: r.index,
        label: SharedString::from(r.label.as_str()),
        percent: r.percent,
        status: SharedString::from(r.status.as_str()),
        running: r.running,
        paused: r.paused,
        bytes_done: SharedString::from(r.bytes_done.as_str()),
        bytes_total: SharedString::from(r.bytes_total.as_str()),
        files_done: r.files_done,
        files_total: r.files_total,
        current_file: SharedString::from(r.current_file.as_str()),
        speed: SharedString::from(r.speed.as_str()),
        speed_peak: SharedString::from(r.speed_peak.as_str()),
        eta: SharedString::from(r.eta.as_str()),
        elapsed: SharedString::from(r.elapsed.as_str()),
        kind: r.kind,
    }
}

/// Categoría de comando de la paleta → el `int` que espera `PaletteItemVm.category`
/// (0=Acción 1=Archivo 2=Reciente 3=Favorito 4=Tema 5=Config). Espeja `CommandCategory`.
fn palette_category_to_int(c: naygo_core::palette::CommandCategory) -> i32 {
    use naygo_core::palette::CommandCategory as Cat;
    match c {
        Cat::Action => 0,
        Cat::File => 1,
        Cat::Recent => 2,
        Cat::Favorite => 3,
        Cat::Theme => 4,
        Cat::Config => 5,
    }
}

/// Mapea los resultados de `filter_and_rank` a la vista: un `PaletteItemVm` por match (con el
/// label partido en segmentos resaltados/normales según `hit_positions`) y, en paralelo, el
/// índice del COMANDO que cada fila ejecuta (para que `on_palette_run(result_idx)` sepa qué
/// comando correr). Devuelve `(items, cmd_indices)` con la misma longitud y orden.
fn palette_items_from_matches(
    commands: &[naygo_core::palette::Command],
    matches: &[naygo_core::palette::CommandMatch],
) -> (Vec<PaletteItemVm>, Vec<usize>) {
    let mut items: Vec<PaletteItemVm> = Vec::with_capacity(matches.len());
    let mut cmd_indices: Vec<usize> = Vec::with_capacity(matches.len());
    for m in matches {
        let Some(cmd) = commands.get(m.index) else {
            continue;
        };
        // Partir el label en runs contiguos: cada char se marca como hit si su índice está en
        // `hit_positions`. Acumulamos en spans alternando el flag `hit`.
        let chars: Vec<char> = cmd.label.chars().collect();
        let hit_set: std::collections::HashSet<usize> = m.hit_positions.iter().copied().collect();
        let mut spans: Vec<PaletteSpanVm> = Vec::new();
        let mut cur = String::new();
        let mut cur_hit: Option<bool> = None;
        for (i, ch) in chars.iter().enumerate() {
            let is_hit = hit_set.contains(&i);
            match cur_hit {
                Some(h) if h == is_hit => cur.push(*ch),
                Some(h) => {
                    spans.push(PaletteSpanVm {
                        text: SharedString::from(cur.as_str()),
                        hit: h,
                    });
                    cur.clear();
                    cur.push(*ch);
                    cur_hit = Some(is_hit);
                }
                None => {
                    cur.push(*ch);
                    cur_hit = Some(is_hit);
                }
            }
        }
        if let Some(h) = cur_hit {
            spans.push(PaletteSpanVm {
                text: SharedString::from(cur.as_str()),
                hit: h,
            });
        }
        items.push(PaletteItemVm {
            spans: ModelRc::from(Rc::new(VecModel::from(spans))),
            category: palette_category_to_int(cmd.category),
            shortcut: SharedString::from(cmd.shortcut.as_str()),
        });
        cmd_indices.push(m.index);
    }
    (items, cmd_indices)
}

fn to_nav_row(r: bridge::NavRow) -> NavRow {
    NavRow {
        label: SharedString::from(r.label.as_str()),
        path: SharedString::from(r.path.as_str()),
        icon: r.icon,
        removable: r.removable,
    }
}

fn to_hist_row(r: bridge::HistRow) -> HistRow {
    HistRow {
        id: r.id as i32,
        label: SharedString::from(r.label.as_str()),
        when: SharedString::from(r.when.as_str()),
        count: r.count,
        undoable: r.undoable,
        reason: SharedString::from(r.reason.as_str()),
    }
}

/// Convierte una ruta de la pila de navegación en una entrada del menú ▾ de historial: `name` es
/// el nombre de la carpeta (o la ruta completa si es raíz, p. ej. "C:\") y `path` la ruta completa
/// atenuada. Lo consumen los menús de Atrás/Adelante de la toolbar.
fn path_to_history_item(p: &std::path::Path) -> HistoryItemVm {
    let display = p.display().to_string();
    let name = p
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| display.clone());
    HistoryItemVm {
        name: SharedString::from(name),
        path: SharedString::from(display),
    }
}

fn to_tree_row(r: bridge::TreeRow) -> TreeRow {
    TreeRow {
        depth: r.depth,
        name: SharedString::from(r.name.as_str()),
        path: SharedString::from(r.path.as_str()),
        expanded: r.expanded,
        has_children: r.has_children,
        is_drive: r.is_drive,
        active: r.active,
        loading: r.loading,
        error: r.error,
        disk_percent: r.disk_percent,
        disk_detail: SharedString::from(r.disk_detail.as_str()),
        icon: r.icon,
    }
}

fn to_fav_tree_row(r: bridge::FavTreeRow) -> FavTreeRow {
    FavTreeRow {
        depth: r.depth,
        is_group: r.is_group,
        name: SharedString::from(r.name.as_str()),
        path: SharedString::from(r.path.as_str()),
        group_id: SharedString::from(r.group_id.as_str()),
        name_path: SharedString::from(r.name_path.as_str()),
        expanded: r.expanded,
        has_children: r.has_children,
        icon: r.icon,
    }
}

fn to_inspector_vm(i: bridge::InspectorInfo) -> InspectorVm {
    InspectorVm {
        present: i.present,
        name: SharedString::from(i.name.as_str()),
        kind: SharedString::from(i.kind.as_str()),
        path: SharedString::from(i.path.as_str()),
        size: SharedString::from(i.size.as_str()),
        modified: SharedString::from(i.modified.as_str()),
        created: SharedString::from(i.created.as_str()),
    }
}
