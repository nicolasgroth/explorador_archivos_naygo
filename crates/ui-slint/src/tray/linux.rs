// Naygo — bandeja Linux mediante Ayatana AppIndicator GLib, cargada en tiempo de ejecución.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// La API upstream no ofrece binding Rust compatible; esta FFI mínima evita enlazar GTK o
// depender de pkg-config al compilar. La biblioteca GPL-3 es opcional en ejecución: si no está
// instalada o no hay sesión D-Bus, Naygo sigue visible y el cierre no oculta la única ventana.
// Todo acceso a D-Bus, carga de bibliotecas y escritura del ícono ocurre en este worker.

use super::TrayMsg;
use libloading::Library;
use naygo_platform::dir_watch::Waker;
use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void, CString};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;

const APP_INDICATOR_CATEGORY_APPLICATION_STATUS: c_int = 0;
const APP_INDICATOR_STATUS_PASSIVE: c_int = 0;
const APP_INDICATOR_STATUS_ACTIVE: c_int = 1;
const G_IO_IN: c_int = 1;
const G_IO_ERR: c_int = 8;
const G_IO_HUP: c_int = 16;
const G_PRIORITY_DEFAULT: c_int = 0;

type MainLoop = *mut c_void;
type GIOCondition = c_int;
type GConnectFlags = c_int;
type GUnixFdSourceFunc = unsafe extern "C" fn(c_int, GIOCondition, *mut c_void) -> c_int;
type GDestroyNotify = unsafe extern "C" fn(*mut c_void);
type GClosureNotify = unsafe extern "C" fn(*mut c_void, *mut c_void);
type GCallback = unsafe extern "C" fn();

type AppIndicatorNewWithPath =
    unsafe extern "C" fn(*const c_char, *const c_char, c_int, *const c_char) -> *mut c_void;
type AppIndicatorSetStatus = unsafe extern "C" fn(*mut c_void, c_int);
type AppIndicatorSetMenu = unsafe extern "C" fn(*mut c_void, *mut c_void);
type AppIndicatorSetActions = unsafe extern "C" fn(*mut c_void, *mut c_void);
type AppIndicatorSetTitle = unsafe extern "C" fn(*mut c_void, *const c_char);
type AppIndicatorSetTooltip =
    unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char);
type AppIndicatorSetSecondaryActivateTarget = unsafe extern "C" fn(*mut c_void, *const c_char);
type GMenuNew = unsafe extern "C" fn() -> *mut c_void;
type GMenuAppend = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char);
type GMenuRemoveAll = unsafe extern "C" fn(*mut c_void);
type GSimpleActionGroupNew = unsafe extern "C" fn() -> *mut c_void;
type GSimpleActionNew = unsafe extern "C" fn(*const c_char, *mut c_void) -> *mut c_void;
type GActionMapAddAction = unsafe extern "C" fn(*mut c_void, *mut c_void);
type GSignalConnectData = unsafe extern "C" fn(
    *mut c_void,
    *const c_char,
    Option<GCallback>,
    *mut c_void,
    Option<GClosureNotify>,
    GConnectFlags,
) -> c_ulong;
type GObjectUnref = unsafe extern "C" fn(*mut c_void);
type GMainLoopNew = unsafe extern "C" fn(*mut c_void, c_int) -> MainLoop;
type GMainLoopRun = unsafe extern "C" fn(MainLoop);
type GMainLoopQuit = unsafe extern "C" fn(MainLoop);
type GMainLoopUnref = unsafe extern "C" fn(MainLoop);
type GUnixFdAddFull = unsafe extern "C" fn(
    c_int,
    c_int,
    GIOCondition,
    Option<GUnixFdSourceFunc>,
    *mut c_void,
    Option<GDestroyNotify>,
) -> c_uint;
type GSourceRemove = unsafe extern "C" fn(c_uint) -> c_int;

struct Api {
    _indicator_library: Library,
    _gio_library: Library,
    _gobject_library: Library,
    _glib_library: Library,
    app_indicator_new_with_path: AppIndicatorNewWithPath,
    app_indicator_set_status: AppIndicatorSetStatus,
    app_indicator_set_menu: AppIndicatorSetMenu,
    app_indicator_set_actions: AppIndicatorSetActions,
    app_indicator_set_title: AppIndicatorSetTitle,
    app_indicator_set_tooltip: AppIndicatorSetTooltip,
    app_indicator_set_secondary_activate_target: AppIndicatorSetSecondaryActivateTarget,
    g_menu_new: GMenuNew,
    g_menu_append: GMenuAppend,
    g_menu_remove_all: GMenuRemoveAll,
    g_simple_action_group_new: GSimpleActionGroupNew,
    g_simple_action_new: GSimpleActionNew,
    g_action_map_add_action: GActionMapAddAction,
    g_signal_connect_data: GSignalConnectData,
    g_object_unref: GObjectUnref,
    g_main_loop_new: GMainLoopNew,
    g_main_loop_run: GMainLoopRun,
    g_main_loop_quit: GMainLoopQuit,
    g_main_loop_unref: GMainLoopUnref,
    g_unix_fd_add_full: GUnixFdAddFull,
    g_source_remove: GSourceRemove,
}

impl Api {
    fn load() -> Result<Arc<Self>, String> {
        let indicator_library = load_library(&[
            "libayatana-appindicator-glib.so.2",
            "libayatana-appindicator-glib.so",
        ])?;
        let gio_library = load_library(&["libgio-2.0.so.0"])?;
        let gobject_library = load_library(&["libgobject-2.0.so.0"])?;
        let glib_library = load_library(&["libglib-2.0.so.0"])?;

        // Los nombres y firmas se contrastaron con headers AppIndicator 2.0.1 y GLib 2.88.
        // SAFETY: cada símbolo se resuelve desde la biblioteca oficial que declara la API.
        unsafe {
            Ok(Arc::new(Self {
                app_indicator_new_with_path: symbol(
                    &indicator_library,
                    b"app_indicator_new_with_path\0",
                )?,
                app_indicator_set_status: symbol(
                    &indicator_library,
                    b"app_indicator_set_status\0",
                )?,
                app_indicator_set_menu: symbol(&indicator_library, b"app_indicator_set_menu\0")?,
                app_indicator_set_actions: symbol(
                    &indicator_library,
                    b"app_indicator_set_actions\0",
                )?,
                app_indicator_set_title: symbol(&indicator_library, b"app_indicator_set_title\0")?,
                app_indicator_set_tooltip: symbol(
                    &indicator_library,
                    b"app_indicator_set_tooltip\0",
                )?,
                app_indicator_set_secondary_activate_target: symbol(
                    &indicator_library,
                    b"app_indicator_set_secondary_activate_target\0",
                )?,
                g_menu_new: symbol(&gio_library, b"g_menu_new\0")?,
                g_menu_append: symbol(&gio_library, b"g_menu_append\0")?,
                g_menu_remove_all: symbol(&gio_library, b"g_menu_remove_all\0")?,
                g_simple_action_group_new: symbol(&gio_library, b"g_simple_action_group_new\0")?,
                g_simple_action_new: symbol(&gio_library, b"g_simple_action_new\0")?,
                g_action_map_add_action: symbol(&gio_library, b"g_action_map_add_action\0")?,
                g_signal_connect_data: symbol(&gobject_library, b"g_signal_connect_data\0")?,
                g_object_unref: symbol(&gobject_library, b"g_object_unref\0")?,
                g_main_loop_new: symbol(&glib_library, b"g_main_loop_new\0")?,
                g_main_loop_run: symbol(&glib_library, b"g_main_loop_run\0")?,
                g_main_loop_quit: symbol(&glib_library, b"g_main_loop_quit\0")?,
                g_main_loop_unref: symbol(&glib_library, b"g_main_loop_unref\0")?,
                g_unix_fd_add_full: symbol(&glib_library, b"g_unix_fd_add_full\0")?,
                g_source_remove: symbol(&glib_library, b"g_source_remove\0")?,
                _indicator_library: indicator_library,
                _gio_library: gio_library,
                _gobject_library: gobject_library,
                _glib_library: glib_library,
            }))
        }
    }
}

fn load_library(names: &[&str]) -> Result<Library, String> {
    let mut last_error = String::new();
    for name in names {
        // SAFETY: the sonames are trusted system libraries; loaded handles are retained in Api.
        match unsafe { Library::new(name) } {
            Ok(library) => return Ok(library),
            Err(error) => last_error = format!("{name}: {error}"),
        }
    }
    Err(last_error)
}

unsafe fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
    // SAFETY: callers request a concrete C ABI signature from the library that exports it.
    unsafe { library.get::<T>(name) }
        .map(|value| *value)
        .map_err(|error| format!("{}: {error}", String::from_utf8_lossy(name)))
}

enum Command {
    SetVisible(bool),
    SetLabels([String; 5]),
    Stop,
}

pub struct LinuxTray {
    commands: Sender<Command>,
    wake_writer: Mutex<UnixStream>,
    running: Arc<AtomicBool>,
    desired_visible: Arc<AtomicBool>,
}

impl LinuxTray {
    fn send(&self, command: Command) -> Result<(), String> {
        if !self.running.load(Ordering::Acquire) {
            return Err("el worker de la bandeja ya terminó".to_string());
        }
        self.commands
            .send(command)
            .map_err(|_| "el worker de la bandeja ya terminó".to_string())?;
        let mut writer = self
            .wake_writer
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match writer.write(&[1]) {
            Ok(1) => Ok(()),
            // Ya hay un wake pendiente; el worker drenará todos los comandos del canal.
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
            Ok(_) => Err("no se pudo despertar el worker de la bandeja".to_string()),
            Err(error) => Err(format!(
                "no se pudo despertar el worker de la bandeja: {error}"
            )),
        }
    }

    pub fn set_visible(&self, visible: bool) -> Result<(), String> {
        self.desired_visible.store(visible, Ordering::Release);
        if let Err(error) = self.send(Command::SetVisible(visible)) {
            self.desired_visible.store(false, Ordering::Release);
            return Err(error);
        }
        Ok(())
    }

    pub fn set_labels(&self, labels: [String; 5]) {
        if let Err(error) = self.send(Command::SetLabels(labels)) {
            crate::logging::log_line(&format!(
                "[tray] no se pudieron actualizar las etiquetas: {error}"
            ));
        }
    }
}

impl Drop for LinuxTray {
    fn drop(&mut self) {
        self.desired_visible.store(false, Ordering::Release);
        let _ = self.send(Command::Stop);
    }
}

pub fn create(labels: [String; 5], waker: Waker) -> Option<(LinuxTray, Receiver<TrayMsg>)> {
    let (reader, writer) = UnixStream::pair().ok()?;
    reader.set_nonblocking(true).ok()?;
    writer.set_nonblocking(true).ok()?;
    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    let running = Arc::new(AtomicBool::new(true));
    let desired_visible = Arc::new(AtomicBool::new(true));
    let connected = Arc::new(AtomicBool::new(false));
    let worker_running = running.clone();
    let worker_desired_visible = desired_visible.clone();
    let worker_connected = connected.clone();
    thread::Builder::new()
        .name("naygo-linux-tray".into())
        .spawn(move || {
            run_worker(
                labels,
                WorkerRuntime {
                    commands: command_rx,
                    reader,
                    event_tx,
                    waker,
                    running: worker_running,
                    desired_visible: worker_desired_visible,
                    connected: worker_connected,
                },
            )
        })
        .ok()?;
    Some((
        LinuxTray {
            commands: command_tx,
            wake_writer: Mutex::new(writer),
            running,
            desired_visible,
        },
        event_rx,
    ))
}

struct ActionData {
    event_tx: Sender<TrayMsg>,
    waker: Waker,
    action: Action,
}

struct ActionContext {
    event_tx: Sender<TrayMsg>,
    waker: Waker,
}

struct ConnectionData {
    event_tx: Sender<TrayMsg>,
    waker: Waker,
    desired_visible: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
}

#[derive(Clone, Copy)]
enum Action {
    Open,
    NewPane,
    Config,
    Center,
    Exit,
}

impl Action {
    fn message(self) -> TrayMsg {
        match self {
            Self::Open => TrayMsg::Open,
            Self::NewPane => TrayMsg::NewPane,
            Self::Config => TrayMsg::OpenConfig,
            Self::Center => TrayMsg::CenterWindow,
            Self::Exit => TrayMsg::Exit,
        }
    }
}

unsafe extern "C" fn action_activated(
    _action: *mut c_void,
    _parameter: *mut c_void,
    data: *mut c_void,
) {
    if data.is_null() {
        return;
    }
    // SAFETY: the signal closure owns this ActionData until its destroy notify runs.
    let data = unsafe { &*(data.cast::<ActionData>()) };
    if data.event_tx.send(data.action.message()).is_ok() {
        (data.waker)();
    }
}

unsafe extern "C" fn destroy_action_data(data: *mut c_void, _closure: *mut c_void) {
    if !data.is_null() {
        // SAFETY: data was allocated by Box::into_raw in add_action and notified once by GObject.
        drop(unsafe { Box::from_raw(data.cast::<ActionData>()) });
    }
}

unsafe extern "C" fn connection_changed(
    _indicator: *mut c_void,
    connected: c_int,
    data: *mut c_void,
) {
    if data.is_null() {
        return;
    }
    // SAFETY: the GObject signal closure owns this data until its destroy notify runs.
    let data = unsafe { &*(data.cast::<ConnectionData>()) };
    let connected = connected != 0;
    data.connected.store(connected, Ordering::Release);
    let visible = connected && data.desired_visible.load(Ordering::Acquire);
    if data.event_tx.send(TrayMsg::Availability(visible)).is_ok() {
        (data.waker)();
    }
}

unsafe extern "C" fn destroy_connection_data(data: *mut c_void, _closure: *mut c_void) {
    if !data.is_null() {
        // SAFETY: data is allocated by Box::into_raw and notified once by GObject.
        drop(unsafe { Box::from_raw(data.cast::<ConnectionData>()) });
    }
}

fn add_action(
    api: &Api,
    menu: *mut c_void,
    actions: *mut c_void,
    name: &str,
    label: &str,
    action: Action,
    context: &ActionContext,
) -> Result<(), String> {
    let name = c_string(name);
    let signal = c_string("activate");
    let label = c_string(label);
    let detailed_action = c_string(&format!("indicator.{}", name.to_string_lossy()));
    // SAFETY: GTK-free Gio constructors accept the C strings above and return owned GObjects.
    let action_object = unsafe { (api.g_simple_action_new)(name.as_ptr(), std::ptr::null_mut()) };
    if action_object.is_null() {
        return Err("Gio no pudo crear una acción de bandeja".to_string());
    }
    let callback: GCallback = unsafe {
        // SAFETY: GCallback is intentionally erased by GObject; the callback has the documented
        // GSimpleAction activate signature (action, parameter, user_data).
        std::mem::transmute::<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void), GCallback>(
            action_activated,
        )
    };
    let data = Box::new(ActionData {
        event_tx: context.event_tx.clone(),
        waker: context.waker.clone(),
        action,
    });
    let data_ptr = Box::into_raw(data).cast::<c_void>();
    // SAFETY: GObject owns the callback data until destroy_action_data runs; action_object is live.
    let signal_id = unsafe {
        (api.g_signal_connect_data)(
            action_object,
            signal.as_ptr(),
            Some(callback),
            data_ptr,
            Some(destroy_action_data),
            0,
        )
    };
    if signal_id == 0 {
        // No handler owns this pointer when GLib returns zero; reclaim it before dropping the
        // action object. For the fixed `activate` signal this is defensive error handling.
        unsafe { (api.g_object_unref)(action_object) };
        unsafe { drop(Box::from_raw(data_ptr.cast::<ActionData>())) };
        return Err("Gio no pudo conectar una acción de bandeja".to_string());
    }
    // SAFETY: the action map retains its own reference; release the constructor's reference after
    // adding it. GMenu copies the label and detailed action strings.
    unsafe {
        (api.g_action_map_add_action)(actions, action_object);
        (api.g_object_unref)(action_object);
        (api.g_menu_append)(menu, label.as_ptr(), detailed_action.as_ptr());
    }
    Ok(())
}

struct LoopState {
    api: Arc<Api>,
    commands: Receiver<Command>,
    reader: UnixStream,
    event_tx: Sender<TrayMsg>,
    waker: Waker,
    running: Arc<AtomicBool>,
    desired_visible: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    loop_ptr: usize,
    indicator: usize,
    menu: usize,
    labels: [String; 5],
}

unsafe extern "C" fn socket_ready(_fd: c_int, condition: c_int, data: *mut c_void) -> c_int {
    if data.is_null() {
        return 0;
    }
    // SAFETY: the source owns a Box<LoopState> until its destroy notify runs.
    let state = unsafe { &mut *data.cast::<LoopState>() };
    if condition & (G_IO_ERR | G_IO_HUP) != 0 {
        state.running.store(false, Ordering::Release);
        (state.api.app_indicator_set_status)(
            state.indicator as *mut c_void,
            APP_INDICATOR_STATUS_PASSIVE,
        );
        notify(state, TrayMsg::Availability(false));
        (state.api.g_main_loop_quit)(state.loop_ptr as MainLoop);
        // Deja el source registrado: el dueño lo elimina una sola vez después de que salga
        // g_main_loop_run. Si devolvemos FALSE, GLib lo destruiría aquí y ese ID ya no es válido.
        return 1;
    }

    let mut bytes = [0_u8; 256];
    loop {
        match state.reader.read(&mut bytes) {
            Ok(0) => {
                state.running.store(false, Ordering::Release);
                notify(state, TrayMsg::Availability(false));
                (state.api.g_main_loop_quit)(state.loop_ptr as MainLoop);
                return 1;
            }
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                crate::logging::log_line(&format!("[tray] error del canal GLib: {error}"));
                state.running.store(false, Ordering::Release);
                notify(state, TrayMsg::Availability(false));
                (state.api.g_main_loop_quit)(state.loop_ptr as MainLoop);
                return 1;
            }
        }
    }

    loop {
        match state.commands.try_recv() {
            Ok(Command::SetVisible(visible)) => {
                state.desired_visible.store(visible, Ordering::Release);
                (state.api.app_indicator_set_status)(
                    state.indicator as *mut c_void,
                    if visible {
                        APP_INDICATOR_STATUS_ACTIVE
                    } else {
                        APP_INDICATOR_STATUS_PASSIVE
                    },
                );
                let connected = state.connected.load(Ordering::Acquire);
                if visible && connected {
                    notify(state, TrayMsg::Availability(true));
                }
            }
            Ok(Command::SetLabels(labels)) => {
                let menu = state.menu as *mut c_void;
                (state.api.g_menu_remove_all)(menu);
                for (label, action) in menu_actions(&labels) {
                    let detailed_action = c_string(&format!("indicator.{action}"));
                    let label = c_string(label);
                    (state.api.g_menu_append)(menu, label.as_ptr(), detailed_action.as_ptr());
                }
                state.labels = labels;
            }
            Ok(Command::Stop) => {
                (state.api.app_indicator_set_status)(
                    state.indicator as *mut c_void,
                    APP_INDICATOR_STATUS_PASSIVE,
                );
                state.running.store(false, Ordering::Release);
                (state.api.g_main_loop_quit)(state.loop_ptr as MainLoop);
                return 1;
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                state.running.store(false, Ordering::Release);
                (state.api.g_main_loop_quit)(state.loop_ptr as MainLoop);
                return 1;
            }
        }
    }
    1
}

fn menu_actions(labels: &[String; 5]) -> [(&str, &'static str); 5] {
    [
        (&labels[0], "open"),
        (&labels[1], "new-pane"),
        (&labels[2], "config"),
        (&labels[3], "center"),
        (&labels[4], "exit"),
    ]
}

fn notify(state: &LoopState, message: TrayMsg) {
    if state.event_tx.send(message).is_ok() {
        (state.waker)();
    }
}

fn c_string(value: &str) -> CString {
    CString::new(value.replace('\0', "�")).expect("NUL reemplazado antes de construir CString")
}

fn prepare_icon() -> Result<PathBuf, String> {
    use image::ImageFormat;
    use std::io::Cursor;
    use std::os::unix::ffi::OsStrExt;

    let image = image::load_from_memory_with_format(
        include_bytes!("../../../../assets/icons/naygo_icon.ico"),
        ImageFormat::Ico,
    )
    .map_err(|error| format!("no se pudo decodificar el ícono: {error}"))?
    .to_rgba8();
    let image = image::imageops::resize(&image, 32, 32, image::imageops::FilterType::Lanczos3);
    let mut encoded = Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|error| format!("no se pudo preparar el ícono PNG: {error}"))?;

    let temp = std::env::temp_dir();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory = temp.join(format!("naygo-tray-{}-{nonce:x}", std::process::id()));
    std::fs::create_dir(&directory)
        .map_err(|error| format!("no se pudo preparar el directorio del ícono: {error}"))?;
    let icon_path = directory.join("naygo.png");
    if let Err(error) = std::fs::write(&icon_path, encoded.into_inner()) {
        let _ = std::fs::remove_dir_all(&directory);
        return Err(format!("no se pudo guardar el ícono de bandeja: {error}"));
    }
    if CString::new(directory.as_os_str().as_bytes()).is_err() {
        let _ = std::fs::remove_dir_all(&directory);
        return Err("la ruta temporal del ícono contiene un byte NUL".to_string());
    }
    Ok(directory)
}

struct WorkerRuntime {
    commands: Receiver<Command>,
    reader: UnixStream,
    event_tx: Sender<TrayMsg>,
    waker: Waker,
    running: Arc<AtomicBool>,
    desired_visible: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
}

fn run_worker(initial_labels: [String; 5], worker: WorkerRuntime) {
    let WorkerRuntime {
        commands,
        reader,
        event_tx,
        waker,
        running,
        desired_visible,
        connected,
    } = worker;
    let api = match Api::load() {
        Ok(api) => api,
        Err(error) => {
            crate::logging::log_line(&format!(
                "[tray] no está disponible Ayatana AppIndicator GLib: {error}"
            ));
            running.store(false, Ordering::Release);
            desired_visible.store(false, Ordering::Release);
            if event_tx.send(TrayMsg::Availability(false)).is_ok() {
                waker();
            }
            return;
        }
    };
    let icon_directory = match prepare_icon() {
        Ok(path) => path,
        Err(error) => {
            crate::logging::log_line(&format!("[tray] {error}"));
            running.store(false, Ordering::Release);
            desired_visible.store(false, Ordering::Release);
            if event_tx.send(TrayMsg::Availability(false)).is_ok() {
                waker();
            }
            return;
        }
    };
    let result = initialize(
        &api,
        &icon_directory,
        &initial_labels,
        &event_tx,
        &waker,
        desired_visible.clone(),
        connected.clone(),
    );
    let (indicator, menu, actions, main_loop) = match result {
        Ok(resources) => resources,
        Err(error) => {
            crate::logging::log_line(&format!(
                "[tray] no se pudo crear AppIndicator GLib: {error}"
            ));
            running.store(false, Ordering::Release);
            desired_visible.store(false, Ordering::Release);
            if event_tx.send(TrayMsg::Availability(false)).is_ok() {
                waker();
            }
            let _ = std::fs::remove_dir_all(&icon_directory);
            return;
        }
    };

    let state = Box::new(LoopState {
        api: api.clone(),
        commands,
        reader,
        event_tx: event_tx.clone(),
        waker: waker.clone(),
        running: running.clone(),
        desired_visible,
        connected,
        loop_ptr: main_loop as usize,
        indicator: indicator as usize,
        menu: menu as usize,
        labels: initial_labels,
    });
    let state_ptr = Box::into_raw(state).cast::<c_void>();
    // `g_unix_fd_add_full` añade un source de lectura al contexto GLib de este worker. No hay
    // polling periódico: el socket despierta el loop solo cuando llega una orden desde la UI.
    // SAFETY: el descriptor y Box<LoopState> viven hasta que GLib retire el source.
    let source_id = unsafe {
        (api.g_unix_fd_add_full)(
            G_PRIORITY_DEFAULT,
            (&*state_ptr.cast::<LoopState>()).reader.as_raw_fd(),
            G_IO_IN | G_IO_ERR | G_IO_HUP,
            Some(socket_ready),
            state_ptr,
            Some(destroy_loop_state),
        )
    };
    if source_id == 0 {
        // GLib no tomó propiedad del callback ni de sus datos.
        unsafe {
            drop(Box::from_raw(state_ptr.cast::<LoopState>()));
            (api.g_object_unref)(indicator);
            (api.g_object_unref)(menu);
            (api.g_object_unref)(actions);
            (api.g_main_loop_unref)(main_loop);
        }
        running.store(false, Ordering::Release);
        let _ = std::fs::remove_dir_all(&icon_directory);
        let _ = event_tx.send(TrayMsg::Availability(false));
        waker();
        return;
    }
    // SAFETY: this worker owns the loop and all its GObjects for the duration of the run.
    unsafe { (api.g_main_loop_run)(main_loop) };
    running.store(false, Ordering::Release);
    // Capture ownership handles before removing the source (its destroy notify frees state).
    unsafe {
        (api.g_source_remove)(source_id);
        (api.g_object_unref)(indicator);
        (api.g_object_unref)(menu);
        (api.g_object_unref)(actions);
        (api.g_main_loop_unref)(main_loop);
    }
    if let Err(error) = std::fs::remove_dir_all(&icon_directory) {
        crate::logging::log_line(&format!(
            "[tray] no se pudo limpiar el ícono temporal: {error}"
        ));
    }
}

unsafe extern "C" fn destroy_loop_state(data: *mut c_void) {
    if !data.is_null() {
        // SAFETY: GLib invokes this notifier once when the fd source is destroyed.
        drop(unsafe { Box::from_raw(data.cast::<LoopState>()) });
    }
}

fn initialize(
    api: &Api,
    icon_directory: &std::path::Path,
    labels: &[String; 5],
    event_tx: &Sender<TrayMsg>,
    waker: &Waker,
    desired_visible: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
) -> Result<(*mut c_void, *mut c_void, *mut c_void, MainLoop), String> {
    use std::os::unix::ffi::OsStrExt;

    let id = c_string("naygo");
    let icon_name = c_string("naygo");
    let icon_path = CString::new(icon_directory.as_os_str().as_bytes())
        .map_err(|_| "la ruta del ícono contiene un byte NUL".to_string())?;
    // SAFETY: the library loaded the documented C ABI and all input strings remain alive for
    // their calls. Ayatana owns its returned indicator reference.
    let indicator = unsafe {
        (api.app_indicator_new_with_path)(
            id.as_ptr(),
            icon_name.as_ptr(),
            APP_INDICATOR_CATEGORY_APPLICATION_STATUS,
            icon_path.as_ptr(),
        )
    };
    if indicator.is_null() {
        return Err("app_indicator_new_with_path devolvió NULL".to_string());
    }
    let menu = unsafe { (api.g_menu_new)() };
    let actions = unsafe { (api.g_simple_action_group_new)() };
    let main_loop = unsafe { (api.g_main_loop_new)(std::ptr::null_mut(), 0) };
    if menu.is_null() || actions.is_null() || main_loop.is_null() {
        unsafe {
            if !menu.is_null() {
                (api.g_object_unref)(menu);
            }
            if !actions.is_null() {
                (api.g_object_unref)(actions);
            }
            (api.g_object_unref)(indicator);
            if !main_loop.is_null() {
                (api.g_main_loop_unref)(main_loop);
            }
        }
        return Err("Gio/GLib no pudo crear el menú o el loop".to_string());
    }

    let action_context = ActionContext {
        event_tx: event_tx.clone(),
        waker: waker.clone(),
    };
    let actions_result = [
        ("open", labels[0].as_str(), Action::Open),
        ("new-pane", labels[1].as_str(), Action::NewPane),
        ("config", labels[2].as_str(), Action::Config),
        ("center", labels[3].as_str(), Action::Center),
        ("exit", labels[4].as_str(), Action::Exit),
    ]
    .into_iter()
    .try_for_each(|(name, label, action)| {
        add_action(api, menu, actions, name, label, action, &action_context)
    });
    if let Err(error) = actions_result {
        unsafe {
            (api.g_object_unref)(menu);
            (api.g_object_unref)(actions);
            (api.g_object_unref)(indicator);
            (api.g_main_loop_unref)(main_loop);
        }
        return Err(error);
    }

    // Solo se considera una vía de retorno real cuando Ayatana confirma la conexión con el
    // watcher del escritorio. Crear el GObject por sí solo no prueba que haya una bandeja visible.
    let signal = c_string("connection-changed");
    let callback: GCallback = unsafe {
        // SAFETY: GCallback erases the documented (indicator, gboolean, user_data) signature.
        std::mem::transmute::<unsafe extern "C" fn(*mut c_void, c_int, *mut c_void), GCallback>(
            connection_changed,
        )
    };
    let connection_data = Box::new(ConnectionData {
        event_tx: event_tx.clone(),
        waker: waker.clone(),
        desired_visible,
        connected,
    });
    let connection_data_ptr = Box::into_raw(connection_data).cast::<c_void>();
    let signal_id = unsafe {
        (api.g_signal_connect_data)(
            indicator,
            signal.as_ptr(),
            Some(callback),
            connection_data_ptr,
            Some(destroy_connection_data),
            0,
        )
    };
    if signal_id == 0 {
        unsafe {
            // The connection-changed handler was not installed, so GObject will not own or
            // destroy its user data when the indicator is unreferenced.
            drop(Box::from_raw(connection_data_ptr.cast::<ConnectionData>()));
            (api.g_object_unref)(menu);
            (api.g_object_unref)(actions);
            (api.g_object_unref)(indicator);
            (api.g_main_loop_unref)(main_loop);
        }
        return Err("Ayatana no permite observar la conexión con el host de bandeja".to_string());
    }

    let title = c_string("Naygo");
    let tooltip = c_string("Naygo");
    let secondary_target = c_string("open");
    unsafe {
        (api.app_indicator_set_menu)(indicator, menu);
        (api.app_indicator_set_actions)(indicator, actions);
        (api.app_indicator_set_title)(indicator, title.as_ptr());
        (api.app_indicator_set_tooltip)(
            indicator,
            icon_name.as_ptr(),
            tooltip.as_ptr(),
            tooltip.as_ptr(),
        );
        (api.app_indicator_set_secondary_activate_target)(indicator, secondary_target.as_ptr());
        (api.app_indicator_set_status)(indicator, APP_INDICATOR_STATUS_ACTIVE);
    }
    Ok((indicator, menu, actions, main_loop))
}
