// Naygo — galería de temas con componentes Slint reales, renderer software y datos sintéticos.
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
    import { FilePanel } from "../ui/file-panel.slint";
    import { ToolbarPath } from "../ui/toolbar-path.slint";
    import { OpButton } from "../ui/ops-panel.slint";
    import { Theme } from "../ui/theme.slint";
    export { Theme }
    export { Tr } from "../ui/i18n.slint";
    export component ThemeProbe inherits Window {
        width: 800px; height: 400px; background: Theme.panel-bg;
        in property <string> name;
        in property <image> sample-icon;
        Text { x: 8px; y: 5px; text: root.name; color: Theme.text; font-size: 18px; }
        ToolbarPath { x: 8px; y: 32px; width: 784px; height: 30px;
            confirmed: "D:\\Ejemplo\\Documentos"; favorite: true; }
        for is-active[i] in [true, false]: FilePanel {
            x: 8px + i * 396px; y: 76px; width: 388px; height: 250px;
            pane-id: i; active: is-active; is-favorite: true;
            segments: [{label:"D:\\",path:"D:\\"},{label:"Ejemplo",path:"D:\\Ejemplo"}];
            columns: [{kind:0,label:"Nombre",width:175px,sort-dir:1},
                {kind:2,label:"Tamaño",width:85px,align-right:true},
                {kind:3,label:"Fecha",width:100px}];
            rows: [
                {name:"Proyectos",is-dir:true,icon:root.sample-icon,cells:["Proyectos","", "2026-09-09"]},
                {name:"seleccionado.txt",selected:true,focused:true,icon:root.sample-icon,cells:["seleccionado.txt","1.2 KB","2026-09-09"]},
                {name:"archivo nuevo.txt",highlight:true,icon:root.sample-icon,cells:["archivo nuevo.txt","3.4 KB","2026-09-09"]},
                {name:"fila alternada.txt",icon:root.sample-icon,cells:["fila alternada.txt","5.6 KB","2026-09-09"]},
                {name:"archivo cortado.txt",cut:true,icon:root.sample-icon,cells:["archivo cortado.txt","7.8 KB","2026-09-09"]}
            ];
            footer-text: "5 elementos"; footer-disk-text: "12 GB libres";
        }
        Text { x:8px;y:336px;text:"Texto secundario · 1.234,56 · 2026-09-09"; color:Theme.text-dim; }
        OpButton { x:8px;y:363px;label:"Pausar";show-pause:true; }
        OpButton { x:112px;y:363px;label:"Saltar";show-skip:true; }
        Text { x:230px;y:367px;text:"Aviso / error";color:Theme.error; }
        Rectangle { x:570px;y:348px;width:220px;height:42px;background:Theme.surface-floating;
            Text {text:"Texto flotante / tooltip";color:Theme.text;horizontal-alignment:center;vertical-alignment:center;} }
    }
}
#[path = "../src/theme_apply.rs"]
mod theme_apply;
struct Headless(Rc<MinimalSoftwareWindow>);

fn contrast(rgb_a: [u8; 3], rgb_b: [u8; 3]) -> f64 {
    let luminance = |rgb: [u8; 3]| {
        let linear = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(rgb[0]) + 0.7152 * linear(rgb[1]) + 0.0722 * linear(rgb[2])
    };
    let a = luminance(rgb_a);
    let b = luminance(rgb_b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

#[test]
fn all_factory_themes_render_real_panels_and_independent_icon_color() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = ThemeProbe::new().unwrap();
    let mut icon = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(16, 16);
    for (index, pixel) in icon.make_mut_slice().iter_mut().enumerate() {
        *pixel = if index % 16 < 8 {
            slint::Rgba8Pixel::new(220, 145, 30, 255)
        } else {
            slint::Rgba8Pixel::new(40, 140, 210, 255)
        };
    }
    ui.set_sample_icon(slint::Image::from_rgba8(icon));
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(800, 400));
    let tmp = tempfile::tempdir().unwrap();
    let catalog = naygo_core::theme::ThemeCatalog::load(
        tmp.path(),
        &naygo_core::theme::ThemeCatalog::default_id(),
    );
    let out =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/agent-out/rp3-themes");
    std::fs::create_dir_all(&out).unwrap();
    for id in catalog.available() {
        let theme = catalog.get(id);
        theme_apply::apply(&ui, theme);
        ui.set_name(theme.name.clone().into());
        slint::platform::update_timers_and_animations();
        window.request_redraw();
        let mut pixels = vec![slint::Rgb8Pixel::default(); 800 * 400];
        window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, 800);
        });
        // Unoccupied footer pixel from the inactive FilePanel; compare against its actual
        // semantic header color, including the software renderer's alpha composition.
        let footer = pixels[318 * 800 + 594];
        let header = ui.global::<Theme>().get_header_bg();
        let footer_rgb = [footer.r, footer.g, footer.b];
        let header_rgb = [header.red(), header.green(), header.blue()];
        let difference: u32 = footer_rgb
            .iter()
            .zip(header_rgb)
            .map(|(a, b)| u32::from(a.abs_diff(b)))
            .sum();
        assert!(
            difference >= 9,
            "{}: footer indistinguishable from header",
            id.as_str()
        );
        let text = [theme.text_dim.r, theme.text_dim.g, theme.text_dim.b];
        // Verify palette tokens on normal, alternating and selected rows as well as common
        // panel/toolbar surfaces. Custom themes are intentionally not constrained here.
        for (surface, color) in [
            ("panel", theme.panel_bg),
            ("toolbar", theme.toolbar_bg),
            ("row", theme.row_bg),
            ("alternate row", theme.row_alt_bg),
            ("selection", theme.selection_bg),
        ] {
            let rgb = [color.r, color.g, color.b];
            assert!(
                contrast(text, rgb) >= 4.5,
                "{}: secondary text on {surface} below 4.5",
                id.as_str()
            );
        }
        println!(
            "footer_contrast {} footer={:.3} header={:.3} primary={:.3}",
            id.as_str(),
            contrast(text, footer_rgb),
            contrast(text, header_rgb),
            contrast([theme.text.r, theme.text.g, theme.text.b], footer_rgb)
        );
        assert!(
            contrast(text, footer_rgb) >= contrast(text, header_rgb) * 0.75,
            "{}: footer tint loses too much text contrast",
            id.as_str()
        );
        let primary = [theme.text.r, theme.text.g, theme.text.b];
        assert!(
            contrast(primary, footer_rgb) >= 4.5,
            "{}: footer text contrast below 4.5",
            id.as_str()
        );
        let bg = pixels[399 * 800 + 799];
        assert_eq!(
            (bg.r, bg.g, bg.b),
            (theme.panel_bg.r, theme.panel_bg.g, theme.panel_bg.b)
        );
        assert_eq!(
            ui.global::<Theme>().get_icon_accent(),
            slint::Color::from_rgb_u8(
                theme.icon_accent.r,
                theme.icon_accent.g,
                theme.icon_accent.b
            )
        );
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
        image::save_buffer(
            out.join(format!("{}.png", id.as_str())),
            &bytes,
            800,
            400,
            image::ColorType::Rgb8,
        )
        .unwrap();
    }
    ui.hide().unwrap();
}

#[test]
fn appearance_keeps_one_icon_catalog_and_stable_category_and_token_ids() {
    let source = include_str!("../ui/config-window.slint");
    assert_eq!(source.matches("in root.vm.icon-set-cards:").count(), 1);
    assert!(source.contains("label: Tr.icons-open-settings;"));
    assert!(source
        .contains("clicked => { root.cat = 9; root.search-text = \"\"; root.capturing = \"\"; }"));
    assert!(source.contains("4, 5, 15,"));
    assert!(
        source.contains("Tr.theme-tok-toolbar-bg,\n        Tr.theme-tok-icon-accent,")
            || source.contains("Tr.theme-tok-toolbar-bg,\r\n        Tr.theme-tok-icon-accent,")
    );
}
