// Naygo — comprobación de fondos de paneles auxiliares con temas de fábrica.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, WindowAdapter,
    },
    ComponentHandle,
};
use std::rc::Rc;

slint::slint! {
    import { InspectorPanel } from "../ui/inspector-panel.slint";
    import { PreviewPanel } from "../ui/preview-panel.slint";
    import { FavoritesPanel } from "../ui/favorites-panel.slint";
    import { BasketPanel } from "../ui/basket-panel.slint";
    import { HistoryPanel } from "../ui/history-panel.slint";
    import { TreePanel } from "../ui/tree-panel.slint";
    import { RecentsPanel } from "../ui/recents-panel.slint";
    import { OpsPanel } from "../ui/ops-panel.slint";
    export { Theme } from "../ui/theme.slint";
    export { Tr } from "../ui/i18n.slint";
    export component AuxProbe inherits Window {
        width: 800px; height: 320px;
        InspectorPanel { x: 0px; y: 0px; width: 200px; height: 160px; }
        PreviewPanel { x: 200px; y: 0px; width: 200px; height: 160px; }
        FavoritesPanel { x: 400px; y: 0px; width: 200px; height: 160px; }
        BasketPanel { x: 600px; y: 0px; width: 200px; height: 160px; }
        HistoryPanel { x: 0px; y: 160px; width: 200px; height: 160px; }
        TreePanel { x: 200px; y: 160px; width: 200px; height: 160px; }
        RecentsPanel { x: 400px; y: 160px; width: 200px; height: 160px; }
        OpsPanel { x: 600px; y: 160px; width: 200px; height: 160px; }
    }
}

#[path = "../src/theme_apply.rs"]
mod theme_apply;
struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

#[test]
fn auxiliary_panels_render_active_background_for_factory_themes() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = AuxProbe::new().unwrap();
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(800, 320));
    let tmp = tempfile::tempdir().unwrap();
    let catalog = naygo_core::theme::ThemeCatalog::load(
        tmp.path(),
        &naygo_core::theme::ThemeCatalog::default_id(),
    );
    for id in catalog.available() {
        let theme = catalog.get(id);
        theme_apply::apply(&ui, theme);
        slint::platform::update_timers_and_animations();
        window.request_redraw();
        let mut pixels = vec![slint::Rgb8Pixel::default(); 800 * 320];
        window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, 800);
        });
        let expected = [
            theme.active_panel_bg.r,
            theme.active_panel_bg.g,
            theme.active_panel_bg.b,
        ];
        for row in 0..2 {
            for col in 0..4 {
                let mut matches = 0;
                for y in row * 160 + 8..row * 160 + 152 {
                    for x in col * 200 + 8..col * 200 + 192 {
                        let p = pixels[y * 800 + x];
                        if [p.r, p.g, p.b] == expected {
                            matches += 1;
                        }
                    }
                }
                assert!(
                    matches > 100,
                    "{}: panel {row},{col} no presenta el fondo activo",
                    id.as_str()
                );
            }
        }
    }
    ui.hide().unwrap();
}
