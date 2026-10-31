// Naygo — ícono en la bandeja del sistema (Slint, Fase 5E). Port del tray de la capa egui.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Tray icon vía el crate `tray-icon` (Tauri, MIT). Se crea en el hilo del event loop y vive
// toda la sesión (drop = el ícono desaparece). Los handlers de eventos corren FUERA del bucle
// de Slint, así que NO tocan la UI: empujan un `TrayMsg` a un canal y despiertan la UI con el
// `waker` (slint::invoke_from_event_loop). El tick drena el canal. Bajo consumo: cero polling.

use naygo_platform::dir_watch::Waker;
use std::sync::mpsc::Receiver;
#[cfg(windows)]
use std::sync::mpsc::{channel, Sender};
#[cfg(windows)]
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
#[cfg(windows)]
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[cfg(target_os = "linux")]
mod linux;

/// Red de seguridad del desmontaje: si un watcher o integración Win32 se atasca en `Drop`, el
/// proceso no puede quedar residente indefinidamente y bloquear una actualización. La sesión se
/// guarda antes de armarla; normalmente el proceso termina mucho antes de este plazo.
pub fn arm_exit_watchdog() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static ARMED: AtomicBool = AtomicBool::new(false);
    if ARMED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("naygo-exit-watchdog".into())
        .spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(4));
            std::process::exit(0);
        });
}

/// Espera las escrituras finales fuera del event loop y luego solicita la salida desde UI.
/// El plazo total es acotado; un disco lento no puede dejar el proceso residente indefinidamente.
pub fn quit_after_persistence(tickets: Vec<naygo_core::config::SaveTicket>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static REQUESTED: AtomicBool = AtomicBool::new(false);
    if REQUESTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("naygo-final-config-save".into())
        .spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            for ticket in tickets {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    crate::logging::log_line("plazo agotado esperando persistencia final");
                    break;
                }
                if let Err(error) = naygo_core::config::wait_settings_saved(&ticket, remaining) {
                    crate::logging::log_line(&format!(
                        "persistencia final incompleta; se continúa el cierre: {error}"
                    ));
                }
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if !remaining.is_zero() {
                let _ = crate::logging::flush(remaining);
            }
            let _ = slint::invoke_from_event_loop(|| {
                arm_exit_watchdog();
                let _ = slint::quit_event_loop();
            });
        });
    if let Err(error) = spawned {
        REQUESTED.store(false, Ordering::SeqCst);
        crate::logging::log_line(&format!(
            "no se pudo iniciar coordinación de cierre: {error}"
        ));
    }
}

/// Mensajes del tray hacia la app (drenados en el tick).
pub enum TrayMsg {
    /// Mostrar + enfocar la ventana principal (clic en el ícono o menú "Abrir").
    Open,
    /// Abrir un panel nuevo (divide el panel activo) y traer la ventana al frente.
    NewPane,
    /// Abrir la ventana de configuración.
    OpenConfig,
    /// Re-centrar la ventana en la pantalla y traerla al frente: rescata una ventana que el
    /// usuario "perdió" (arrastrada fuera de la pantalla, minimizada, o tapada).
    CenterWindow,
    /// Salir de verdad (menú "Salir"), aunque `close_to_tray` esté activo.
    Exit,
    /// El backend asíncrono cambió su disponibilidad/visibilidad. `true` confirma que el
    /// indicador fue creado y se pidió estado activo; la presencia en el panel aún requiere
    /// aceptación manual del escritorio.
    #[cfg(target_os = "linux")]
    Availability(bool),
}

pub struct Tray {
    /// Mantiene vivo el ícono (drop = desaparece de la bandeja).
    #[cfg(windows)]
    icon: TrayIcon,
    #[cfg(windows)]
    open_item: MenuItem,
    #[cfg(windows)]
    new_pane_item: MenuItem,
    #[cfg(windows)]
    config_item: MenuItem,
    #[cfg(windows)]
    center_item: MenuItem,
    #[cfg(windows)]
    exit_item: MenuItem,
    #[cfg(target_os = "linux")]
    backend: linux::LinuxTray,
    pub rx: Receiver<TrayMsg>,
}

impl Tray {
    /// En Windows, la creación es síncrona y `Some` implica un ícono ya creado. Linux inicializa
    /// el servicio GLib en worker; empieza en `false` hasta recibir `Availability(true)`.
    pub fn active_on_start(&self) -> bool {
        #[cfg(windows)]
        {
            true
        }
        #[cfg(target_os = "linux")]
        {
            false
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            false
        }
    }

    /// Cambia visibilidad al aplicar `tray_enabled` desde la configuración cargada en worker.
    pub fn set_visible(&self, visible: bool) -> Result<(), String> {
        #[cfg(windows)]
        {
            self.icon
                .set_visible(visible)
                .map_err(|error| error.to_string())
        }
        #[cfg(target_os = "linux")]
        {
            self.backend.set_visible(visible)
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = visible;
            Err("bandeja no disponible en esta plataforma".to_string())
        }
    }

    /// Actualiza las etiquetas/atajos al aplicar idioma y keymap persistidos desde el worker.
    pub fn set_labels(&self, labels: [String; 5]) {
        #[cfg(windows)]
        {
            self.open_item.set_text(&labels[0]);
            self.new_pane_item.set_text(&labels[1]);
            self.config_item.set_text(&labels[2]);
            self.center_item.set_text(&labels[3]);
            self.exit_item.set_text(&labels[4]);
        }
        #[cfg(target_os = "linux")]
        self.backend.set_labels(labels);
        #[cfg(not(any(windows, target_os = "linux")))]
        let _ = labels;
    }

    /// Quita el ícono de la bandeja de forma INMEDIATA (Shell_NotifyIcon NIM_DELETE síncrono, vía
    /// `set_visible(false)` del crate). Se llama al SALIR de verdad, ANTES de `quit_event_loop`:
    /// si solo se dejara caer el `Drop` al terminar el proceso, Windows deja el ícono "fantasma"
    /// en la barra hasta que el usuario pasa el mouse por encima y el SO lo repinta. Tolerante.
    pub fn hide_icon(&self) {
        #[cfg(windows)]
        {
            let _ = self.icon.set_visible(false);
        }
        #[cfg(target_os = "linux")]
        {
            let _ = self.backend.set_visible(false);
        }
    }
}

/// Crea el ícono de bandeja con su menú. Tolerante: `None` si algo falla (la app sigue normal,
/// solo sin tray). `waker` despierta la UI cuando llega un evento del tray. Re-instala los
/// handlers globales para que apunten al canal de ESTA instancia.
pub fn create(
    open_label: &str,
    new_pane_label: &str,
    config_label: &str,
    center_label: &str,
    exit_label: &str,
    waker: Waker,
) -> Option<Tray> {
    #[cfg(windows)]
    {
        let icon = load_icon()?;
        let menu = Menu::new();
        let open_item = MenuItem::new(open_label, true, None);
        let new_pane_item = MenuItem::new(new_pane_label, true, None);
        let config_item = MenuItem::new(config_label, true, None);
        let center_item = MenuItem::new(center_label, true, None);
        let exit_item = MenuItem::new(exit_label, true, None);
        menu.append(&open_item).ok()?;
        menu.append(&new_pane_item).ok()?;
        menu.append(&config_item).ok()?;
        menu.append(&center_item).ok()?;
        menu.append(&PredefinedMenuItem::separator()).ok()?;
        menu.append(&exit_item).ok()?;

        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Naygo")
            .with_menu(Box::new(menu))
            .build()
            .map_err(|e| eprintln!("[tray] no se pudo crear el tray: {e}"))
            .ok()?;

        let (tx, rx): (Sender<TrayMsg>, Receiver<TrayMsg>) = channel();

        // Clic IZQUIERDO (al soltar) o DOBLE-CLIC izquierdo sobre el ícono → abrir/enfocar. El
        // doble-clic se maneja aparte porque Windows lo entrega como evento propio (tras el segundo
        // clic llega `DoubleClick`, no otro `Click`): sin este brazo, el segundo clic del usuario
        // se perdería. Ambos gestos hacen lo mismo (abrir), como Steam/Teams/OneDrive.
        let tx_click = tx.clone();
        let waker_click = waker.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            let open = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            );
            if open {
                let _ = tx_click.send(TrayMsg::Open);
                waker_click();
            }
        }));

        // Menú contextual: Abrir / Nuevo panel / Centrar ventana / Salir.
        let open_id = open_item.id().clone();
        let new_pane_id = new_pane_item.id().clone();
        let config_id = config_item.id().clone();
        let center_id = center_item.id().clone();
        let exit_id = exit_item.id().clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let msg = if event.id == open_id {
                Some(TrayMsg::Open)
            } else if event.id == new_pane_id {
                Some(TrayMsg::NewPane)
            } else if event.id == config_id {
                Some(TrayMsg::OpenConfig)
            } else if event.id == center_id {
                Some(TrayMsg::CenterWindow)
            } else if event.id == exit_id {
                Some(TrayMsg::Exit)
            } else {
                None
            };
            if let Some(m) = msg {
                let _ = tx.send(m);
                waker();
            }
        }));

        Some(Tray {
            icon: tray,
            open_item,
            new_pane_item,
            config_item,
            center_item,
            exit_item,
            rx,
        })
    }
    #[cfg(target_os = "linux")]
    {
        let (backend, rx) = linux::create(
            [
                open_label.to_string(),
                new_pane_label.to_string(),
                config_label.to_string(),
                center_label.to_string(),
                exit_label.to_string(),
            ],
            waker,
        )?;
        Some(Tray { backend, rx })
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (
            open_label,
            new_pane_label,
            config_label,
            center_label,
            exit_label,
            waker,
        );
        None
    }
}

/// Decodifica el `.ico` embebido y lo reescala a 32×32 RGBA para la bandeja.
#[cfg(windows)]
fn load_icon() -> Option<tray_icon::Icon> {
    let bytes = include_bytes!("../../../assets/icons/naygo_icon.ico");
    // Si el ícono no carga, se registra en el log (antes iba a stderr, invisible en una
    // instalación): un tray sin ícono es una de las causas de que el tray no llegue a crearse.
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Ico)
        .map_err(|e| crate::logging::log_line(&format!("[tray] ícono ilegible: {e}")))
        .ok()?
        .to_rgba8();
    let img = image::imageops::resize(&img, 32, 32, image::imageops::FilterType::Lanczos3);
    tray_icon::Icon::from_rgba(img.into_raw(), 32, 32)
        .map_err(|e| crate::logging::log_line(&format!("[tray] ícono inválido: {e}")))
        .ok()
}

/// ¿La app debe salir cuando se cierra su única ventana?
/// `close_to_tray` solo puede ocultarla si existe un camino real para volver a mostrarla:
/// bandeja activa o hotkey global registrado. En Linux el hotkey todavía no está soportado, por
/// eso no se debe minimizar la ventana si la bandeja no llegó a crearse. En Windows cualquiera de
/// las dos vías puede servir; si ambas fallan, salir es más seguro que dejar el proceso inaccesible.
pub fn should_quit_on_close(
    close_to_tray: bool,
    tray_active: bool,
    global_hotkey_active: bool,
) -> bool {
    !close_to_tray || (!tray_active && !global_hotkey_active)
}

#[cfg(test)]
mod tests {
    use super::should_quit_on_close;

    #[test]
    fn cierre_a_bandeja_requiere_una_via_de_retorno() {
        // Sin preferencia de bandeja, la X sale siempre.
        assert!(should_quit_on_close(false, true, true));
        assert!(should_quit_on_close(false, false, false));
        // La bandeja o el hotkey registrado bastan para volver.
        assert!(!should_quit_on_close(true, true, false));
        assert!(!should_quit_on_close(true, false, true));
        // Sin ambos, salir evita dejar una instancia oculta e inaccesible.
        assert!(should_quit_on_close(true, false, false));
    }
}
