// Naygo — arranque de la capa UI en Slint (Fase 2b: multi-panel + paneles especiales).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
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
mod callbacks_config;
mod callbacks_ctx;
mod callbacks_history;
mod callbacks_layouts;
mod callbacks_listing;
mod callbacks_nav;
mod callbacks_ops;
mod callbacks_palette;
mod callbacks_panes;
mod callbacks_pathbar;
mod callbacks_refresh;
mod callbacks_rename;
mod config_ctrl;
mod devices;
mod i18n_keys;
mod icons;
mod keys;
mod listing;
mod logging;
mod models;
mod ops_ctrl;
mod packs;
mod preview;
mod sync;
mod theme_apply;
mod tick;
mod tray;
mod vm_builders;
mod watch;
mod win_helpers;
mod wire;
mod workspace_ctrl;

use models::Models;
use vm_builders::*;
use win_helpers::*;

use naygo_core::workspace::layout::Rect;
use naygo_core::workspace::{PaneId, PanePurpose};
use slint::{ModelRc, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use workspace_ctrl::WorkspaceCtrl;

slint::include_modules!();

/// CHANGELOG embebido en build time: fuente de la sección "Novedades" del Acerca de.
/// Ruta relativa desde este archivo (crates/ui-slint/src/) hasta la raíz del repo.
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// Versión completa mostrada al usuario: `X.Y.Z+build.YYYYMMDDHHMM`. El sufijo `+build.<id>` es el
/// metadato de build de semver (no altera la versión semver base); lo estampa `build.rs` en
/// `NAYGO_BUILD_ID` en cada compilación, así cada build es identificable sin ambigüedad al probar.
/// Si el id es "unknown" (no se pudo leer la hora en el build), se muestra solo la versión base.
fn naygo_full_version() -> String {
    let base = env!("CARGO_PKG_VERSION");
    match option_env!("NAYGO_BUILD_ID") {
        Some(id) if id != "unknown" && !id.is_empty() => format!("{base}+build.{id}"),
        _ => base.to_string(),
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
                naygo_full_version()
            ))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
        return Ok(());
    }

    // INSTANCIA ÚNICA: si Naygo ya corre en esta sesión, NO se abre un segundo proceso — se le
    // avisa a la instancia viva que se muestre (y que abra la carpeta pedida, si venía una en la
    // línea de comandos) y este proceso termina en silencio. Es el comportamiento estándar de
    // las apps de bandeja (Steam/Teams/OneDrive): el ícono anclado, el acceso directo o el menú
    // "Abrir en Naygo" reutilizan la instancia viva en vez de duplicar procesos. Va ANTES de
    // crear cualquier ventana (ni el splash ni la AppWindow deben llegar a parpadear). El guard
    // vive hasta el final de main(); su hilo vigilante se arranca más abajo, cuando existe el
    // waker del loop de UI (`si_guard.watch`).
    let si_guard = match naygo_platform::single_instance::acquire() {
        naygo_platform::single_instance::Instance::AlreadyRunning => {
            logging::log_line("instancia única: ya hay un Naygo corriendo; se le avisa y salimos");
            naygo_platform::single_instance::notify_running(cli_args.dir.as_deref());
            return Ok(());
        }
        naygo_platform::single_instance::Instance::Primary(guard) => guard,
    };

    let ui = AppWindow::new()?;
    // Título de la ventana limpio: solo "Naygo". El id de build (p. ej. "0.3.0+build.202607021614")
    // se muestra en el Acerca de (vía `set_app_version`) y en el splash de arranque, no en la barra
    // de título ni en la barra de tareas.
    ui.set_window_title("Naygo".into());
    let start = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:/"));
    let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new(start)));
    // Proveedor de metadata de versión de exe/dll (Win32 VerQueryValue). Se registra una
    // sola vez al arrancar, antes de que la UI pueda pedir metadata de un archivo.
    #[cfg(windows)]
    naygo_core::metadata::register_provider(Box::new(naygo_platform::exe_meta::ExeMeta));
    // El registro (HKCU\...\Run) es la fuente de verdad de `autostart`, no settings.json: el
    // instalador puede crear la entrada Run sin pasar por la UI (o el usuario puede borrarla a
    // mano). Sincronizamos el ajuste guardado contra el registro real al arrancar.
    #[cfg(windows)]
    {
        let reg_on = naygo_platform::autostart::is_enabled();
        let mut c = ctrl.borrow_mut();
        if c.config.settings.autostart != reg_on {
            c.config.settings.autostart = reg_on;
            c.config.save();
        }
    }

    // Hotkey global (mostrar/ocultar Naygo desde cualquier app). El registro se mantiene vivo en
    // este slot durante toda la ejecución (drop = se libera). `hotkey_id` guarda el id para
    // reconocer sus eventos en el tick.
    let global_hotkey_slot: Rc<RefCell<Option<naygo_platform::global_hotkey::GlobalHotkey>>> =
        Rc::new(RefCell::new(None));
    let hotkey_id: Rc<std::cell::Cell<Option<u32>>> = Rc::new(std::cell::Cell::new(None));

    // (Re)arma el hotkey global según la config actual. Suelta el registro anterior, y si está
    // activo re-registra. Devuelve Err si el SO lo rechaza (el llamador decide si avisar). Se usa
    // tanto en el registro inicial como desde los handlers de Config (toggle + recaptura).
    let rearm_hotkey: Rc<dyn Fn() -> Result<(), String>> = {
        let ctrl = ctrl.clone();
        let slot = global_hotkey_slot.clone();
        let id_cell = hotkey_id.clone();
        Rc::new(move || -> Result<(), String> {
            *slot.borrow_mut() = None; // soltar el registro anterior primero
            id_cell.set(None);
            let s = ctrl.borrow().config.settings.clone();
            if !s.global_hotkey_enabled {
                return Ok(());
            }
            #[cfg(windows)]
            {
                match naygo_platform::global_hotkey::register(&s.global_hotkey) {
                    Ok(h) => {
                        id_cell.set(Some(h.id()));
                        *slot.borrow_mut() = Some(h);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            #[cfg(not(windows))]
            {
                Ok(())
            }
        })
    };
    // Registro inicial (un fallo solo se loguea, sin modal: no queremos molestar cada arranque).
    if let Err(e) = rearm_hotkey() {
        logging::log_line(&format!("no se pudo registrar el hotkey global: {e}"));
    } else if hotkey_id.get().is_some() {
        logging::log_line("hotkey global registrado");
    }

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
    // Inicializar las etiquetas y mensajes de error del preview con el idioma activo al arranque.
    {
        let labels = archive_labels_from_config(&ctrl.borrow().config);
        let msgs = preview_msgs_from_config(&ctrl.borrow().config);
        ctrl.borrow_mut().preview.set_archive_labels(labels);
        ctrl.borrow_mut().preview.set_preview_msgs(msgs);
    }
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

    // Splash de arranque (Fase 5F): solo en release, y NUNCA en un arranque por autostart a la
    // bandeja (`--tray`): al iniciar sesión de Windows no debe aparecer NADA de Naygo, ni
    // siquiera el splash — solo el ícono junto al reloj. Ventana breve de bienvenida que se
    // cierra sola a ~1.8s. La ventana principal se construye por detrás (el splash no la
    // bloquea). En debug se omite (arranque directo). Se mantiene vivo en una variable de main.
    // Nota: el Splash usa los colores POR DEFECTO del global Theme (azul marino), que coinciden
    // con el tema default — no hace falta aplicarle el tema activo (es una pantalla efímera).
    #[cfg(not(debug_assertions))]
    let _splash_keepalive = if cli_args.tray {
        None
    } else {
        match Splash::new() {
            Ok(splash) => {
                // Muestra el id de build al pie del splash (misma fuente que el Acerca de).
                splash.set_build_version(naygo_full_version().into());
                let _ = splash.show();
                // Centrar en la pantalla primaria: una Window suelta de Slint abre por defecto
                // arriba a la izquierda. Tamaño del splash (de splash.slint): 360x220 lógicos.
                let (sw, sh) = naygo_platform::window::primary_screen_size();
                splash.window().set_position(slint::LogicalPosition::new(
                    ((sw as f32 - 360.0) / 2.0).max(0.0),
                    ((sh as f32 - 220.0) / 2.0).max(0.0),
                ));
                let splash = Rc::new(splash);
                // El splash debe quedar ENCIMA de la ventana principal, que se muestra casi a la vez.
                // `set_topmost` (SetWindowPos HWND_TOPMOST) lo eleva sin robarle el foco ni moverlo. El
                // detalle clave: ANTES de `ui.run()` la ventana del splash NO está realizada por winit y
                // NO tiene HWND todavía (medido: `splash_hwnd` devuelve `None` hasta ~1 s después de
                // entrar al event loop). Por eso el topmost se aplica desde un `Timer` que corre en el
                // hilo de UI YA con el loop andando: sondea cada 100 ms y, en cuanto obtiene el HWND, lo
                // eleva UNA vez y se auto-detiene (`topmost_timer.stop()`). Es robusto ante equipos
                // lentos (una VM podría tardar más en realizar la ventana): sigue reintentando hasta
                // lograrlo, sin costo perceptible (el splash vive solo ~1.8 s).
                let splash_topmost = splash.clone();
                let topmost_timer = Rc::new(slint::Timer::default());
                let topmost_timer_self = topmost_timer.clone();
                topmost_timer.start(
                    slint::TimerMode::Repeated,
                    std::time::Duration::from_millis(100),
                    move || {
                        if let Some(hwnd) = splash_hwnd(&splash_topmost) {
                            naygo_platform::window::set_topmost(hwnd);
                            topmost_timer_self.stop();
                        }
                    },
                );
                let splash_for_timer = splash.clone();
                let timer = slint::Timer::default();
                timer.start(
                    slint::TimerMode::SingleShot,
                    std::time::Duration::from_millis(1800),
                    move || {
                        let _ = splash_for_timer.hide();
                    },
                );
                Some((splash, timer, topmost_timer))
            }
            Err(_) => None,
        }
    };

    let models = Rc::new(RefCell::new(Models::new()));

    ui.set_panes(ModelRc::from(models.borrow().panes.clone()));
    ui.set_splits(ModelRc::from(models.borrow().splits.clone()));
    ui.set_picks(ModelRc::from(models.borrow().picks.clone()));

    let area_of: Rc<dyn Fn() -> Rect> = Rc::new({
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
    });

    // Última firma de filas por panel Files (clave: PaneId.0). `sync_rows` compara la firma de
    // cada panel contra esta tabla y, si no cambió, NO reconstruye sus filas (O-1: evita decenas
    // de miles de allocs de String por tick en carpetas grandes bajo render por software). Vive
    // junto a `models` y lo captura el closure; no va en el controlador para no enredar los
    // préstamos (`sync_rows` ya tiene `c`/`m` prestados).
    let last_row_sig: Rc<RefCell<HashMap<u64, u64>>> = Rc::new(RefCell::new(HashMap::new()));

    // Actualiza SOLO el contenido (filas + structs + flags) sin tocar la estructura. Barato:
    // corre en cada tick. Mantiene los mismos VecModel → los ListView conservan su scroll.
    // `sync_layout` (estructural) reconcilia la lista de paneles y splitters. Ambos se
    // construyen en `sync::build_sync` (extraídos de aquí por tamaño de archivo).
    let sync::SyncHandles {
        sync_rows,
        sync_layout,
    } = sync::build_sync(
        ui.as_weak(),
        ctrl.clone(),
        models.clone(),
        last_row_sig.clone(),
        area_of.clone(),
    );

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

    // Instancia única: hilo vigilante del evento "muéstrate" que disparan las instancias
    // secundarias (segundo lanzamiento del exe). Marca el flag y despierta el loop; el tick lo
    // drena: restaura la ventana y, si la secundaria dejó una carpeta en el spool ("Abrir en
    // Naygo" con la app ya corriendo), la abre en un panel nuevo.
    let si_show_requested = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    si_guard.watch(si_show_requested.clone(), waker.clone());

    // Hotkey global: handler que despierta el loop de UI en cada pulsación. Sin esto, con la app
    // dormida (reposo/bajo consumo) la pulsación quedaba encolada hasta el próximo wake por otra
    // causa y el atajo "no respondía". Se instala UNA vez; el re-registro por cambio de
    // combinación en Config no lo invalida (el handler no filtra por id).
    naygo_platform::global_hotkey::install_wake_handler(waker.clone());

    // One-shot de la restauración de geometría guardada: la aplica `try_restore_saved_geometry`
    // en el PRIMER momento en que la ventana está VISIBLE (ver esa función por el porqué del
    // gate). Se intenta desde el tick (≤30 ms tras cualquier show) y desde `on_wake` — el
    // primero que la encuentre visible la aplica y consume el flag.
    #[cfg(windows)]
    let geometry_restored = std::rc::Rc::new(std::cell::Cell::new(false));

    // Watcher de dispositivos (Fase 5B): detecta USB enchufado/quitado. Vive toda la sesión.
    let devices = Rc::new(devices::Devices::start(waker.clone()));
    // HOME para reubicar paneles cuya unidad desapareció.
    let home: Rc<std::path::PathBuf> = Rc::new(
        std::env::var_os("USERPROFILE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("C:/")),
    );

    // Aplica un cambio de unidades (USB enchufado/quitado): drena el watcher, reubica paneles
    // huérfanos y RECONSTRUYE la tira de discos de la toolbar (construido en `sync.rs`).
    let apply_device_change =
        sync::build_apply_device_change(ctrl.clone(), devices.clone(), home.clone(), ui.as_weak());

    // Drag&drop OLE — RECIBIR (Fase 5D): canal de archivos soltados sobre la ventana. El
    // registro del IDropTarget se hace en el primer tick (cuando el HWND ya es válido) y el
    // guard vive toda la sesión. Por simplicidad el drop va al panel ACTIVO (fallback del
    // diseño: no se mapea el punto de drop a un panel concreto).
    let (drop_tx, drop_rx) = std::sync::mpsc::channel::<naygo_platform::drop_target::DropPayload>();
    let drop_rx = Rc::new(drop_rx);
    // Canal HERMANO para los eventos de HOVER durante el arrastre (resaltar el panel bajo el
    // cursor: borde + título). Separado de `drop_*` a propósito: el hover es de alta frecuencia y
    // visual; el drop es el commit. El `IDropTarget` emite `Over`/`Leave` por aquí (ver
    // drop_target.rs) y despierta la UI con el mismo waker; el tick lo drena abajo.
    let (drag_tx, drag_rx) = std::sync::mpsc::channel::<naygo_platform::drop_target::DragHover>();
    let drag_rx = Rc::new(drag_rx);
    let drop_guard: Rc<RefCell<Option<naygo_platform::drop_target::DropTargetGuard>>> =
        Rc::new(RefCell::new(None));

    // Tray (Fase 5E): ícono en bandeja con menú Abrir/Salir, solo si el ajuste lo pide. Vive
    // toda la sesión. `tray_active` lo lee el handler de cierre para decidir si oculta a la
    // bandeja o sale de verdad.
    let tray: Rc<Option<tray::Tray>> = Rc::new(if ctrl.borrow().config.settings.tray_enabled {
        let t = {
            let c = ctrl.borrow();
            // Cada opción del menú muestra su atajo a la derecha (didáctico). En los menús nativos de
            // Windows, un `\t` alinea el texto que le sigue a la derecha. Componemos el atajo aquí
            // (donde está el keymap + el hotkey global), no en tray.rs, que solo recibe los labels.
            // Reusamos los formateadores ya existentes: `chord_to_text` (para el hotkey global) y
            // `chord_text_for` (primer chord de una acción del keymap; vacío si no tiene atajo).
            let with_shortcut = |label: String, shortcut: &str| -> String {
                if shortcut.is_empty() {
                    label
                } else {
                    format!("{label}\t{shortcut}")
                }
            };
            // Abrir → hotkey global (restaura la ventana), solo si está habilitado.
            let open_shortcut = if c.config.settings.global_hotkey_enabled {
                config_ctrl::ConfigCtrl::chord_to_text(&c.config.settings.global_hotkey)
            } else {
                String::new()
            };
            // Nuevo panel → Action::SplitPanel; Configuración → Action::OpenConfig. Centrar ventana y
            // Salir no tienen acción con chord configurable, así que van sin atajo (solo el label).
            let new_pane_shortcut = c
                .config
                .chord_text_for(naygo_core::keymap::Action::SplitPanel);
            let config_shortcut = c
                .config
                .chord_text_for(naygo_core::keymap::Action::OpenConfig);
            tray::create(
                &with_shortcut(c.config.t("slint.tray.open"), &open_shortcut),
                &with_shortcut(c.config.t("slint.tray.new_pane"), &new_pane_shortcut),
                &with_shortcut(c.config.t("slint.tray.config"), &config_shortcut),
                &c.config.t("slint.tray.center"),
                &c.config.t("slint.tray.exit"),
                waker.clone(),
            )
        };
        // Si el tray estaba pedido pero no se pudo crear, dejar constancia en el log: sin este
        // aviso el fallo era invisible. Ya no afecta al cierre (la X respeta close_to_tray aunque
        // el tray falle), pero explica por qué no aparece el ícono en la bandeja.
        // Si el tray estaba pedido pero no se pudo crear, dejar constancia en el log (sin este aviso
        // el fallo era invisible). No afecta al cierre (la X respeta close_to_tray aunque el tray
        // falle), pero explica por qué no aparece el ícono de bandeja.
        if t.is_none() {
            crate::logging::log_line(
                "[tray] tray_enabled=true pero la creación del tray falló; sin ícono de bandeja",
            );
        }
        t
    } else {
        None
    });
    let tray_active = tray.is_some();

    // Timer que drena listados de archivos + árbol + preview; se apaga cuando todo está en
    // reposo (0 trabajo). El preview cambia structs del PaneVm → en cada tick sync_rows.
    // El cuerpo del tick vive en `tick.rs` (extraído de aquí por tamaño de archivo).
    let timer = Rc::new(slint::Timer::default());
    let start_timer = tick::build_start_timer(tick::TickDeps {
        ctrl: ctrl.clone(),
        sync_rows: sync_rows.clone(),
        sync_layout: sync_layout.clone(),
        timer: timer.clone(),
        apply_device_change: apply_device_change.clone(),
        waker: waker.clone(),
        ui_weak: ui.as_weak(),
        drop_tx: drop_tx.clone(),
        drop_rx: drop_rx.clone(),
        drag_tx: drag_tx.clone(),
        drag_rx: drag_rx.clone(),
        drop_guard: drop_guard.clone(),
        tray: tray.clone(),
        tray_active,
        hotkey_id: hotkey_id.clone(),
        #[cfg(windows)]
        geometry_restored: geometry_restored.clone(),
        #[cfg(not(windows))]
        geometry_restored: Rc::new(std::cell::Cell::new(false)),
        si_show_requested: si_show_requested.clone(),
    });
    start_timer();

    // `wake`: lo dispara un worker (watcher de carpeta/dispositivos) vía invoke_from_event_loop.
    // Corre en el hilo de UI. Además de re-arrancar el timer, aplica YA un posible cambio de
    // unidades: así un USB recién conectado refresca la tira de discos EN VIVO aunque el timer
    // estuviera dormido (el refresh no depende de que llegue un tick).
    {
        let start_timer = start_timer.clone();
        let apply_device_change = apply_device_change.clone();
        let ui_weak_wake = ui.as_weak();
        #[cfg(windows)]
        let ctrl_wake = ctrl.clone();
        let logged_first_wake = std::rc::Rc::new(std::cell::Cell::new(false));
        #[cfg(windows)]
        let geometry_restored = geometry_restored.clone();
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
            // Geometría guardada: mismo one-shot que en el tick (el primero que encuentre la
            // ventana visible la aplica). Nota histórica: aquí vivía también el "esconder la
            // ventana si venimos de autostart --tray"; se eliminó porque el wake se dispara
            // recién con la actividad de los watchers (puede tardar minutos) — el arranque a
            // bandeja ahora directamente NO muestra la ventana (ver el show condicional antes
            // de `run_event_loop_until_quit`).
            #[cfg(windows)]
            if let Some(ui) = ui_weak_wake.upgrade() {
                try_restore_saved_geometry(&ui, &ctrl_wake, &geometry_restored);
            }
            apply_device_change();
            start_timer();
        });
    }

    // Estado pendiente de confirmaciones modales (expulsar USB / deshacer): celdas compartidas
    // entre los handlers que abren el modal y el dispatcher `on_message_confirm`/`cancel`.
    // Path pendiente de expulsar mientras el modal de confirmación está abierto.
    let pending_eject: std::rc::Rc<std::cell::RefCell<Option<String>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    // Ids de los paneles a soltar (cerrar watcher) antes de expulsar, mientras el modal está abierto.
    let pending_eject_panes: std::rc::Rc<std::cell::RefCell<Vec<u64>>> =
        std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    // Id de la entrada del historial a DESHACER mientras el popup de confirmación (MessageVm kind 5)
    // está abierto. El botón "Deshacer" ya NO ejecuta directo: abre el popup con el detalle de lo
    // que se hará y guarda aquí el id; el deshacer real ocurre en `on_message_confirm` (kind 5).
    let pending_undo: std::rc::Rc<std::cell::RefCell<Option<u64>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));

    // Dependencias compartidas del cableado de callbacks (los `wire_*` de los módulos
    // `callbacks_*` las reciben como un solo struct; el cableado vive fuera de `main` por
    // tamaño de archivo, en el MISMO orden en que estaba aquí).
    let wctx = wire::WireCtx {
        ctrl: ctrl.clone(),
        sync_rows: sync_rows.clone(),
        sync_layout: sync_layout.clone(),
        start_timer: start_timer.clone(),
        area_of: area_of.clone(),
        palette_cmds: palette_cmds.clone(),
        palette_cmd_indices: palette_cmd_indices.clone(),
        pending_eject: pending_eject.clone(),
        pending_eject_panes: pending_eject_panes.clone(),
        pending_undo: pending_undo.clone(),
    };
    // Listado de archivos: clics, drag-out, rubber-band, orden y menú de columnas.
    callbacks_listing::wire_listing(&ui, &wctx);
    // Teclado global del panel + atrás/adelante del mouse.
    callbacks_nav::wire_nav_keys(&ui, &wctx);
    // Rename inline (F2): commit / encadenar / cancelar.
    callbacks_rename::wire_rename(&ui, &wctx);
    // Subir / atrás / adelante / home / agregar paneles.
    callbacks_nav::wire_nav_go(&ui, &wctx);

    // --- Ventana de configuración (Fase 4) ---
    // Closures de refresco (VM de config, íconos de toolbar, discos, plantillas): se construyen
    // en `callbacks_refresh.rs` y se llaman una vez aquí para poblar la UI al arrancar.
    let refresh_config_vm =
        callbacks_refresh::build_refresh_config_vm(ctrl.clone(), ui.as_weak(), cfg_win.as_weak());
    refresh_config_vm();
    let refresh_toolbar_icons =
        callbacks_refresh::build_refresh_toolbar_icons(ctrl.clone(), ui.as_weak());
    refresh_toolbar_icons();
    let refresh_drives = callbacks_refresh::build_refresh_drives(ctrl.clone(), ui.as_weak());
    refresh_drives();
    let refresh_layouts = callbacks_refresh::build_refresh_layouts(ctrl.clone(), ui.as_weak());
    refresh_layouts();
    // Plantillas de disposición + batch-rename + ayuda.
    callbacks_layouts::wire_layouts(&ui, &wctx, &refresh_layouts);
    // Paleta de comandos (Ctrl+P).
    callbacks_palette::wire_palette(&ui, &cfg_win, &wctx);
    // Ventana de Configuración (todos sus callbacks) + el engranaje que la abre.
    callbacks_config::wire_config(
        &ui,
        &cfg_win,
        &wctx,
        &refresh_config_vm,
        &refresh_toolbar_icons,
        &refresh_drives,
        &rearm_hotkey,
    );
    // Árbol / historial / menús de toolbar / discos / dispatcher del MessageModal.
    callbacks_history::wire_history(&ui, &wctx, &refresh_drives);
    // Path-bar (breadcrumbs + edición con autocompletado) y árbol de favoritos.
    callbacks_pathbar::wire_pathbar(&ui, &wctx);
    // Operaciones de archivo y sus diálogos modales.
    callbacks_ops::wire_ops(&ui, &wctx);
    // Menú contextual, carpetas nuevas, "carpeta no encontrada", búsqueda, toolbar.
    callbacks_ctx::wire_ctx_menu(&ui, &wctx);
    // Multi-panel: swap/clone/stack, pestañas, drag de paneles, splitters, resize.
    callbacks_panes::wire_panes(&ui, &wctx);

    // Al cerrar la ventana (Fase 5E, arregla la deuda de F4): persistir la sesión y luego SALIR
    // DE VERDAD (quit_event_loop), salvo que el usuario haya pedido "cerrar a bandeja" y el tray
    // esté activo, en cuyo caso se oculta a la bandeja y la app sigue viva a propósito.
    {
        let ctrl = ctrl.clone();
        let ui_weak_close = ui.as_weak();
        ui.window().on_close_requested(move || {
            // Capturar y persistir la geometría de la ventana ANTES de save_session/salir. Va en
            // su propio scope para que el borrow_mut() suelte `c` antes de los ctrl.borrow() que
            // siguen.
            #[cfg(windows)]
            {
                let mut c = ctrl.borrow_mut();
                if let Some(ui) = ui_weak_close.upgrade() {
                    if let Some(hwnd) = naygo_hwnd(&ui) {
                        if let Some(p) = naygo_platform::window_geometry::get(hwnd) {
                            c.config.settings.window = Some(naygo_core::config::WindowGeometry {
                                width: p.width,
                                height: p.height,
                                x: p.x,
                                y: p.y,
                                maximized: p.maximized,
                            });
                        }
                    }
                }
                c.config.save();
            }
            ctrl.borrow().save_session();
            let close_to_tray = ctrl.borrow().config.settings.close_to_tray;
            let quit = tray::should_quit_on_close(close_to_tray, tray_active);
            if quit {
                // Salir de verdad: terminar el loop y dejar que Slint oculte la ventana.
                let _ = slint::quit_event_loop();
                slint::CloseRequestResponse::HideWindow
            } else {
                // Ir a la BANDEJA sin matar la app. CLAVE: NO se puede responder `HideWindow` — al
                // ocultar la única ventana visible, Slint baja su contador de ventanas a 0 y
                // TERMINA el event loop (el proceso muere). Por eso se responde `KeepWindowShown`
                // (mantiene la app viva) y se esconde a mano por Win32:
                // `set_taskbar_visible(false)` quita el botón de la barra de tareas
                // (WS_EX_TOOLWINDOW) y hace `SW_HIDE` — la ventana desaparece del todo y queda
                // solo el ícono de la bandeja del reloj. OJO: NO llamar `set_minimized(true)`
                // encima — minimizar una ventana WS_EX_TOOLWINDOW la RE-MUESTRA como un
                // mini-título flotante pegado a la barra de tareas (estilo Win 3.x): era el
                // "cuadrito" fantasma arrastrable y con doble-clic que restauraba una ventana
                // sin botones Max/Min. El botón de barra se devuelve al restaurar (ícono de la
                // bandeja, atajo global o relanzar el exe → `restore_window`).
                if let Some(ui) = ui_weak_close.upgrade() {
                    #[cfg(windows)]
                    if let Some(hwnd) = naygo_hwnd(&ui) {
                        naygo_platform::window::set_taskbar_visible(hwnd, false);
                    }
                    // En no-Windows no existe set_taskbar_visible: minimizar es el mejor esfuerzo
                    // (y ahí minimizar NO produce el mini-título de Win32).
                    #[cfg(not(windows))]
                    ui.window().set_minimized(true);
                }
                slint::CloseRequestResponse::KeepWindowShown
            }
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
    // Arranque a BANDEJA (autostart --tray + la opción activada + tray vivo): la ventana NO se
    // muestra en absoluto — ni flash, ni botón de barra de tareas, ni nada; solo el ícono junto
    // al reloj (como Steam/Teams/OneDrive). Antes se mostraba y se intentaba esconder en el
    // primer `on_wake`, pero ese wake se dispara recién con la PRIMERA INTERACCIÓN del usuario
    // (medido: minutos después), así que la ventana quedaba visible. La clave es
    // `run_event_loop_until_quit()`: a diferencia de `ui.run()` (que muestra la ventana y
    // termina el loop cuando la última visible se cierra), corre el loop AUNQUE no haya ninguna
    // ventana visible, hasta `quit_event_loop()`. La ventana se materializa recién cuando el
    // usuario la pide (ícono de bandeja, atajo global o relanzar el exe → `restore_window`).
    // Si NO hay tray activo, `start_in_tray` es false y se muestra normal (sin tray, una ventana
    // oculta sería irrecuperable con el mouse).
    let start_in_tray =
        cli_args.tray && ctrl.borrow().config.settings.autostart_minimized && tray_active;
    if !start_in_tray {
        ui.show()?;
    } else {
        crate::logging::log_line("arranque directo a bandeja: ventana sin mostrar");
    }
    slint::run_event_loop_until_quit()
}

/// Devuelve el RGB que deben usar los íconos tintables: el color de texto del tema activo,
/// salvo que el usuario haya fijado un color explícito con `toolbar_glyph_color`.
pub(crate) fn theme_text_rgb(
    settings: &naygo_core::config::Settings,
    themes: &naygo_core::theme::ThemeCatalog,
) -> (u8, u8, u8) {
    if let Some(c) = settings.toolbar_glyph_color {
        return (c.r, c.g, c.b);
    }
    let t = themes.get(&settings.theme);
    (t.text.r, t.text.g, t.text.b)
}
