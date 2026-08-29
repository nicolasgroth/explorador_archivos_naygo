// Naygo — ícono en la bandeja del sistema (Slint, Fase 5E). Port del tray de la capa egui.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Tray icon vía el crate `tray-icon` (Tauri, MIT). Se crea en el hilo del event loop y vive
// toda la sesión (drop = el ícono desaparece). Los handlers de eventos corren FUERA del bucle
// de Slint, así que NO tocan la UI: empujan un `TrayMsg` a un canal y despiertan la UI con el
// `waker` (slint::invoke_from_event_loop). El tick drena el canal. Bajo consumo: cero polling.

use naygo_platform::dir_watch::Waker;
use std::sync::mpsc::{channel, Receiver, Sender};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

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
}

pub struct Tray {
    /// Mantiene vivo el ícono (drop = desaparece de la bandeja).
    icon: TrayIcon,
    pub rx: Receiver<TrayMsg>,
}

impl Tray {
    /// Quita el ícono de la bandeja de forma INMEDIATA (Shell_NotifyIcon NIM_DELETE síncrono, vía
    /// `set_visible(false)` del crate). Se llama al SALIR de verdad, ANTES de `quit_event_loop`:
    /// si solo se dejara caer el `Drop` al terminar el proceso, Windows deja el ícono "fantasma"
    /// en la barra hasta que el usuario pasa el mouse por encima y el SO lo repinta. Tolerante.
    pub fn hide_icon(&self) {
        let _ = self.icon.set_visible(false);
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

    Some(Tray { icon: tray, rx })
}

/// Decodifica el `.ico` embebido y lo reescala a 32×32 RGBA para la bandeja.
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

/// ¿La app debe SALIR al cerrar la ventana? Sale SOLO si el usuario NO pidió "cerrar a bandeja".
/// Si sí lo pidió (`close_to_tray`), la ventana se oculta y la app sigue viva, AUNQUE el tray no se
/// haya podido crear: matar el proceso porque el ícono de bandeja falló al cargar sería la peor
/// opción (el usuario perdería la app sin querer). Con la ventana oculta y sin tray, la app sigue
/// recuperable por el hotkey global (Ctrl+Alt+Z por defecto) que restaura la ventana. Antes esta
/// decisión dependía de `tray_active`, y si la creación del tray fallaba en silencio, la X cerraba
/// la app.
/// `tray_active` se conserva en la firma solo para el diagnóstico del llamador (loguearlo), no
/// decide el cierre.
pub fn should_quit_on_close(close_to_tray: bool, _tray_active: bool) -> bool {
    !close_to_tray
}

#[cfg(test)]
mod tests {
    use super::should_quit_on_close;

    #[test]
    fn cierre_sale_solo_si_no_pidio_bandeja() {
        // Sin "cerrar a bandeja": la X sale, haya tray o no.
        assert!(should_quit_on_close(false, true));
        assert!(should_quit_on_close(false, false));
        // Con "cerrar a bandeja": NUNCA sale, aunque el tray haya fallado (tray_active=false).
        // Antes este caso cerraba la app; ahora se oculta y sigue viva.
        assert!(!should_quit_on_close(true, false));
        assert!(!should_quit_on_close(true, true));
    }
}
