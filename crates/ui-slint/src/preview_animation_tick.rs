// Naygo — reloj exclusivo del preview: no reconstruye listados, árbol ni workspace.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use crate::{models::Models, workspace_ctrl::WorkspaceCtrl, AppWindow};
#[cfg(not(windows))]
use slint::ComponentHandle;
use slint::Model;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

pub(crate) fn build(
    ctrl: Rc<RefCell<WorkspaceCtrl>>,
    models: Rc<RefCell<Models>>,
    ui: slint::Weak<AppWindow>,
) -> Rc<dyn Fn()> {
    let timer = Rc::new(slint::Timer::default());
    Rc::new(move || {
        if timer.running() || !ctrl.borrow().preview.animation_tick_needed() {
            return;
        }
        let ctrl = ctrl.clone();
        let models = models.clone();
        let ui = ui.clone();
        let weak_timer = Rc::downgrade(&timer);
        let pending = Rc::new(Cell::new(false));
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(timer) = weak_timer.upgrade() else {
                    return;
                };
                let Some(ui) = ui.upgrade() else {
                    timer.stop();
                    return;
                };
                let Ok(mut c) = ctrl.try_borrow_mut() else {
                    return;
                };
                let now = Instant::now();
                #[cfg(windows)]
                let visible = crate::win_helpers::naygo_hwnd(&ui)
                    .is_some_and(naygo_platform::window::content_visible);
                #[cfg(not(windows))]
                let visible = ui.window().is_visible();
                if !visible {
                    c.preview.set_visible(false, now);
                }
                // Si `models` está prestado (reentrada del loop modal de arrastre), el frame no se
                // publica en este tick: queda pendiente y el timer sigue vivo para reintentarlo,
                // igual que cuando `ctrl` está prestado (arriba). Sin esto, el último frame (p. ej.
                // el mensaje que termina la animación) quedaba sin publicar hasta otra interacción,
                // porque el tick principal puede estar dormido.
                if c.preview.poll_animation(now) {
                    pending.set(true);
                }
                if pending.get() && publish_preview(&c, &models) {
                    pending.set(false);
                }
                if !pending.get() && !c.preview.animation_tick_needed() {
                    timer.stop();
                }
            },
        );
    })
}

/// Publica el VM de vista previa en los paneles Preview. Devuelve `false` si `models` está
/// prestado y hay que reintentar en el próximo tick.
fn publish_preview(c: &WorkspaceCtrl, models: &RefCell<Models>) -> bool {
    let Ok(mut m) = models.try_borrow_mut() else {
        return false;
    };
    let preview = m.preview_vm.vm(c);
    for (index, id) in m.pane_ids.iter().enumerate() {
        if c.ws
            .pane(*id)
            .is_some_and(|p| p.purpose == naygo_core::workspace::PanePurpose::Preview)
        {
            if let Some(mut pane) = m.panes.row_data(index) {
                if !crate::persistent_models::preview_vm_eq(&pane.preview, &preview) {
                    pane.preview = preview.clone();
                    m.panes.set_row_data(index, pane);
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publicar_con_models_prestado_pide_reintento() {
        let temp = tempfile::tempdir().unwrap();
        let ctrl = WorkspaceCtrl::new_in(temp.path().to_path_buf(), temp.path().join("settings"));
        let models = RefCell::new(Models::new());
        {
            let _busy = models.borrow();
            assert!(
                !publish_preview(&ctrl, &models),
                "con `models` prestado el frame queda pendiente"
            );
        }
        assert!(publish_preview(&ctrl, &models));
    }
}
