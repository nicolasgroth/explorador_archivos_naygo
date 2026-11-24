// Naygo — contraste y compatibilidad de paletas RP-3, sin UI ni preferencias personales.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;

fn luminance(c: ThemeColor) -> f64 {
    let channel = |v: u8| {
        let s = f64::from(v) / 255.;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}
fn contrast(a: ThemeColor, b: ThemeColor) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn six_light_palettes_are_builtin_readable_and_do_not_replace_the_default() {
    let dir = tempfile::tempdir().unwrap();
    let cat = ThemeCatalog::load(dir.path(), &ThemeId::new("dracula"));
    assert_eq!(cat.available().len(), BUILTIN_THEME_IDS.len());
    assert_eq!(ThemeCatalog::default_id().as_str(), "dark-blue");
    assert_eq!(cat.get(&ThemeId::new("dracula")).name, "Dracula");
    for id in [
        "isgroth-nieve",
        "isgroth-perla",
        "isgroth-arena",
        "isgroth-salvia",
        "isgroth-bruma",
        "isgroth-grafito-claro",
    ] {
        assert!(is_builtin_id(id));
        let theme = cat.get(&ThemeId::new(id));
        assert_eq!(theme.base, ThemeBase::Light);
        assert_ne!(theme.highlight, theme.icon_accent);
        for bg in [
            theme.panel_bg,
            theme.toolbar_bg,
            theme.active_panel_bg,
            theme.inactive_panel_bg,
            theme.row_bg,
            theme.row_alt_bg,
            theme.row_inactive_bg,
            theme.selection_bg,
        ] {
            assert!(contrast(theme.text, bg) >= 4.5, "{id}: main text");
            assert!(contrast(theme.text_dim, bg) >= 4.5, "{id}: secondary text");
            assert!(
                contrast(theme.icon_accent, bg) >= 3.,
                "{id}: emphasized icon"
            );
        }
    }
}

#[test]
fn old_theme_files_keep_their_foreground_and_new_token_round_trips() {
    let old = Theme::from_json(r##"{"base":"light","highlight":"#124578"}"##).unwrap();
    assert_eq!(old.highlight, old.icon_accent);
    let mut edited = old.clone();
    edited.icon_accent = ThemeColor::new(30, 40, 50);
    let saved = serde_json::to_string(&edited).unwrap();
    let restored = Theme::from_json(&saved).unwrap();
    assert_eq!(restored, edited);
    assert_eq!(restored.highlight, old.highlight);
    let empty = Theme::from_json("{}").unwrap();
    assert_eq!(empty.highlight, empty.icon_accent);
}

#[test]
fn malformed_unicode_color_is_an_error_not_a_slice_panic() {
    for bad in ["a€bc", "€abc", "ééé", "#a€bc"] {
        assert_eq!(ThemeColor::from_hex(bad), None);
    }
    assert!(Theme::from_json(r##"{"icon_accent":"#a€bc"}"##).is_none());
}
