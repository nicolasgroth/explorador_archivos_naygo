// Naygo — ícono en la bandeja del sistema (junto al reloj).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Tray icon vía el crate `tray-icon` (Tauri, MIT). Se crea en el hilo del event
//! loop y vive en `NaygoApp` (drop = el ícono desaparece). Los handlers de eventos
//! corren fuera del frame de egui, así que NO tocan la app: empujan un `TrayMsg` a
//! un canal y despiertan la UI con `request_repaint()` (thread-safe); `logic()`
//! drena el canal en el próximo frame. Bajo consumo: cero polling.

use std::sync::mpsc::{channel, Receiver, Sender};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// Mensajes del tray hacia la app (drenados en `logic`).
pub enum TrayMsg {
    /// Mostrar + enfocar la ventana principal (clic en el ícono o menú "Abrir").
    Open,
    /// Salir de verdad (menú "Salir"), aunque `close_to_tray` esté activo.
    Exit,
}

pub struct Tray {
    /// Mantiene vivo el ícono (drop = desaparece de la bandeja).
    _icon: TrayIcon,
    pub rx: Receiver<TrayMsg>,
}

/// Crea el ícono de bandeja con su menú. Tolerante: `None` si algo falla (la app
/// sigue normal, solo sin tray). Re-instala los handlers globales de eventos para
/// que apunten al canal de ESTA instancia (sobreviven a toggles on/off).
pub fn create(ctx: &egui::Context, open_label: &str, exit_label: &str) -> Option<Tray> {
    let icon = load_icon()?;
    let menu = Menu::new();
    let open_item = MenuItem::new(open_label, true, None);
    let exit_item = MenuItem::new(exit_label, true, None);
    menu.append(&open_item).ok()?;
    menu.append(&PredefinedMenuItem::separator()).ok()?;
    menu.append(&exit_item).ok()?;

    let tray = TrayIconBuilder::new()
        .with_icon(icon)
        .with_tooltip("Naygo")
        .with_menu(Box::new(menu))
        .build()
        .map_err(|e| tracing::warn!("no se pudo crear el tray: {e}"))
        .ok()?;

    let (tx, rx): (Sender<TrayMsg>, Receiver<TrayMsg>) = channel();

    // Clic IZQUIERDO (al soltar) sobre el ícono → abrir/enfocar.
    let tx_click = tx.clone();
    let ctx_click = ctx.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            let _ = tx_click.send(TrayMsg::Open);
            ctx_click.request_repaint();
        }
    }));

    // Menú contextual: Abrir / Salir.
    let open_id = open_item.id().clone();
    let exit_id = exit_item.id().clone();
    let ctx_menu = ctx.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let msg = if event.id == open_id {
            Some(TrayMsg::Open)
        } else if event.id == exit_id {
            Some(TrayMsg::Exit)
        } else {
            None
        };
        if let Some(m) = msg {
            let _ = tx.send(m);
            ctx_menu.request_repaint();
        }
    }));

    Some(Tray { _icon: tray, rx })
}

/// Decodifica el `.ico` embebido y lo reescala a 32×32 RGBA para la bandeja.
fn load_icon() -> Option<tray_icon::Icon> {
    let bytes = include_bytes!("../../../assets/icons/naygo_icon.ico");
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Ico)
        .map_err(|e| tracing::warn!("ícono del tray ilegible: {e}"))
        .ok()?
        .to_rgba8();
    let img = image::imageops::resize(&img, 32, 32, image::imageops::FilterType::Lanczos3);
    tray_icon::Icon::from_rgba(img.into_raw(), 32, 32)
        .map_err(|e| tracing::warn!("ícono del tray inválido: {e}"))
        .ok()
}
