// Naygo — estabilidad del viewport del árbol ante cambios de índice y navegación.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, WindowAdapter, WindowEvent,
    },
    ComponentHandle, LogicalPosition, ModelRc,
};
use std::rc::Rc;

#[path = "../src/tree_rows_model.rs"]
mod tree_rows_model;
use tree_rows_model::TreeRowsModel;

slint::slint! {
    import { TreePanel } from "../ui/tree-panel.slint";
    import { TreeRow } from "../ui/types.slint";
    export { TreeRow }
    export component TreeScrollProbe inherits Window {
        width: 420px; height: 300px;
        in property <[TreeRow]> rows;
        in property <int> reveal-row: -1;
        in property <string> reveal-key;
        out property <float> offset: panel.scroll-offset / 1px;
        panel := TreePanel {
            width: 100%; height: 100%; rows: root.rows; row-h: 22px;
            reveal-row: root.reveal-row; reveal-key: root.reveal-key;
        }
    }
}

struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

fn pump(window: &MinimalSoftwareWindow) {
    for _ in 0..4 {
        slint::platform::update_timers_and_animations();
        window.draw_if_needed(|renderer| {
            let mut pixels = vec![slint::Rgb8Pixel::default(); 420 * 360];
            renderer.render(&mut pixels, 420);
        });
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn tree_keeps_scroll_on_index_change_but_reveals_explicit_navigation() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = TreeScrollProbe::new().unwrap();
    let make_row = |i: usize| TreeRow {
        name: format!("folder-{i}").into(),
        path: format!("fixture/{i}").into(),
        ..Default::default()
    };
    let rows = Rc::new(TreeRowsModel::default());
    rows.reconcile((0..100).map(make_row).collect());
    ui.set_rows(ModelRc::from(rows.clone()));
    ui.set_reveal_key("fixture/30".into());
    ui.set_reveal_row(30);
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(420, 300));
    pump(&window);
    let initial = ui.get_offset();
    let position = LogicalPosition::new(180.0, 150.0);
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    ui.window().dispatch_event(WindowEvent::PointerScrolled {
        position,
        delta_x: 0.0,
        delta_y: -700.0,
    });
    pump(&window);
    let before = ui.get_offset();
    assert!(
        (before - initial).abs() > 30.0,
        "la rueda debe desplazar el árbol: {initial} → {before}"
    );
    rows.reconcile((0..100).map(make_row).collect());
    pump(&window);
    assert!(
        (ui.get_offset() - before).abs() < 1.0,
        "reemplazar las mismas filas no debe recentrar: before={before}, after={}",
        ui.get_offset()
    );
    let mut replacement = vec![make_row(0)];
    replacement[0].expanded = true;
    replacement.push(make_row(999));
    replacement.extend((1..100).map(make_row));
    rows.reconcile(replacement);
    ui.set_reveal_row(31);
    pump(&window);
    let replaced = ui.get_offset();
    assert!(
        ((replaced - before) + 22.0).abs() < 1.0,
        "expandir arriba del viewport debe conservar la misma carpeta visible"
    );
    rows.reconcile((0..100).map(make_row).collect());
    ui.set_reveal_row(30);
    pump(&window);
    assert!(
        (ui.get_offset() - before).abs() < 1.0,
        "quitar esa fila restaura el ancla"
    );

    ui.set_reveal_key("fixture/80".into());
    ui.set_reveal_row(80);
    pump(&window);
    assert!(
        (ui.get_offset() - before).abs() > 100.0,
        "una navegación nueva debe revelar su ruta"
    );
    let before_resize = ui.get_offset();
    window.set_size(slint::PhysicalSize::new(420, 360));
    pump(&window);
    assert!(
        (ui.get_offset() - before_resize).abs() < 1.0,
        "el cambio de altura no debe centrar de nuevo"
    );
    ui.window().dispatch_event(WindowEvent::PointerScrolled {
        position,
        delta_x: 0.0,
        delta_y: 400.0,
    });
    pump(&window);
    let manually_scrolled = ui.get_offset();
    assert!((manually_scrolled - before_resize).abs() > 30.0);
    let mut replacement: Vec<_> = (0..100).map(make_row).collect();
    replacement[80] = make_row(998);
    rows.reconcile(replacement);
    ui.set_reveal_key("fixture/998".into());
    pump(&window);
    assert!(
        (ui.get_offset() - manually_scrolled).abs() > 30.0,
        "una ruta distinta con el mismo índice debe revelarse"
    );
    ui.set_reveal_key("fixture/90".into());
    ui.set_reveal_row(-1);
    pump(&window);
    let waiting = ui.get_offset();
    ui.set_reveal_row(90);
    pump(&window);
    assert!(
        (ui.get_offset() - waiting).abs() > 100.0,
        "la ruta tardía se revela cuando aparece"
    );
}
