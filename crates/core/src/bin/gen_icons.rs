// Naygo — generador build-time de los sets de íconos de fábrica (SVG -> PNG).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// No forma parte del binario de la app. Se ejecuta a mano para regenerar
// assets/icons/<set>/. Lee los SVG directamente de los .zip de las librerías y
// rasteriza cada IconKey a PNG (Task 6). Idempotente; nunca toca el config del usuario.

/// Una entrada de mapeo: clave estable de Naygo -> nombre del SVG dentro de la librería.
// Los campos se usan en Task 6 (rasterización); el allow evita warnings en el stub.
#[allow(dead_code)]
struct Map {
    key: &'static str,
    svg: &'static str,
}

/// Especificación de un set: id, zip fuente, prefijo dentro del zip, tintable, mapeo.
// Los campos se usan en Task 6 (rasterización); el allow evita warnings en el stub.
#[allow(dead_code)]
struct SetSpec {
    id: &'static str,
    zip: &'static str,   // ruta relativa al zip, p.ej. "assets/icons/lucide-main.zip"
    prefix: &'static str, // prefijo dentro del zip, p.ej. "lucide-main/icons/"
    tintable: bool,
    gray: bool,          // mono = true (se tiñe gris en Task 6)
    maps: &'static [Map],
}

// ---------------------------------------------------------------------------
// Lucide (https://lucide.dev). MIT License.
// Estructura: lucide-main/icons/<name>.svg
// ---------------------------------------------------------------------------
const LUCIDE: &[Map] = &[
    Map { key: "folder",           svg: "folder" },
    Map { key: "drive",            svg: "hard-drive" },
    Map { key: "unknown",          svg: "file" },
    Map { key: "file_image",       svg: "file-image" },
    // lucide no tiene file-video.svg; el más cercano es file-video-camera
    Map { key: "file_video",       svg: "file-video-camera" },
    Map { key: "file_audio",       svg: "file-music" },
    Map { key: "file_document",    svg: "file-text" },
    Map { key: "file_code",        svg: "file-code" },
    Map { key: "file_archive",     svg: "file-archive" },
    // lucide no tiene file-executable; file-terminal es lo más representativo
    Map { key: "file_executable",  svg: "file-terminal" },
    // box.svg existe en lucide (verificado)
    Map { key: "file_model3d",     svg: "box" },
    // lucide no tiene file-font; type.svg (la letra T) es el sustituto tipográfico
    Map { key: "file_font",        svg: "type" },
    Map { key: "file_generic",     svg: "file" },
    Map { key: "action_back",      svg: "arrow-left" },
    Map { key: "action_forward",   svg: "arrow-right" },
    Map { key: "action_up",        svg: "arrow-up" },
    Map { key: "action_refresh",   svg: "refresh-cw" },
    Map { key: "action_copy",      svg: "copy" },
    Map { key: "action_cut",       svg: "scissors" },
    Map { key: "action_paste",     svg: "clipboard" },
    Map { key: "action_delete",    svg: "trash" },
    Map { key: "action_new_file",  svg: "file-plus" },
    Map { key: "action_new_folder",svg: "folder-plus" },
    // panel-left como "agregar panel" (añade columna izquierda)
    Map { key: "action_add_pane",  svg: "panel-left" },
    Map { key: "action_swap_panes",svg: "arrow-left-right" },
    // copy-plus = "clonar ruta / duplicar panel"
    Map { key: "action_clone_path",svg: "copy-plus" },
    Map { key: "action_new_window",svg: "app-window" },
    Map { key: "action_settings",  svg: "settings" },
    // lucide no tiene tabs.svg; file-stack (apilar archivos) es el sustituto
    Map { key: "action_tabs",      svg: "file-stack" },
    Map { key: "action_layouts",   svg: "layout-panel-left" },
    Map { key: "action_terminal",  svg: "terminal" },
    // lucide no tiene eject.svg; disc.svg es el CD/disco expulsable más cercano
    Map { key: "action_eject",     svg: "disc" },
    Map { key: "action_panel",     svg: "panel-right" },
];

// ---------------------------------------------------------------------------
// Tabler Icons (https://tabler.io/icons). MIT License.
// Estructura: tabler-icons-main/icons/outline/<name>.svg
// ---------------------------------------------------------------------------
const TABLER: &[Map] = &[
    Map { key: "folder",           svg: "folder" },
    // tabler no tiene hard-drive.svg; device-desktop (monitor de escritorio) es lo más cercano
    Map { key: "drive",            svg: "device-desktop" },
    Map { key: "unknown",          svg: "file-unknown" },
    // tabler no tiene file-image; photo.svg es el equivalente
    Map { key: "file_image",       svg: "photo" },
    Map { key: "file_video",       svg: "video" },
    Map { key: "file_audio",       svg: "file-music" },
    Map { key: "file_document",    svg: "file-text" },
    Map { key: "file_code",        svg: "file-code" },
    // tabler no tiene file-archive; file-zip es el equivalente comprimido
    Map { key: "file_archive",     svg: "file-zip" },
    // tabler no tiene file-executable ni file-terminal; file-code-2 es lo más técnico
    Map { key: "file_executable",  svg: "file-code-2" },
    // tabler tiene file-3d (verificado)
    Map { key: "file_model3d",     svg: "file-3d" },
    // tabler no tiene file-font; typography.svg (letra T con serifa) es el sustituto
    Map { key: "file_font",        svg: "typography" },
    Map { key: "file_generic",     svg: "file" },
    Map { key: "action_back",      svg: "arrow-left" },
    Map { key: "action_forward",   svg: "arrow-right" },
    Map { key: "action_up",        svg: "arrow-up" },
    Map { key: "action_refresh",   svg: "refresh" },
    Map { key: "action_copy",      svg: "copy" },
    Map { key: "action_cut",       svg: "scissors" },
    // tabler no tiene clipboard-paste; clipboard.svg es el sustituto genérico
    Map { key: "action_paste",     svg: "clipboard" },
    Map { key: "action_delete",    svg: "trash" },
    Map { key: "action_new_file",  svg: "file-plus" },
    Map { key: "action_new_folder",svg: "folder-plus" },
    // columns-2 = dos columnas (agregar panel lateral)
    Map { key: "action_add_pane",  svg: "columns-2" },
    // arrows-exchange = intercambiar paneles
    Map { key: "action_swap_panes",svg: "arrows-exchange" },
    // copy-plus = clonar ruta en panel nuevo
    Map { key: "action_clone_path",svg: "copy-plus" },
    Map { key: "action_new_window",svg: "app-window" },
    Map { key: "action_settings",  svg: "settings" },
    // tabler no tiene tabs.svg; layout-sidebar es el más cercano para gestión de paneles
    Map { key: "action_tabs",      svg: "layout-sidebar" },
    Map { key: "action_layouts",   svg: "layout-columns" },
    Map { key: "action_terminal",  svg: "terminal" },
    // tabler no tiene eject.svg; player-eject existe (verificado)
    Map { key: "action_eject",     svg: "player-eject" },
    // layout-sidebar-right = panel derecho visible
    Map { key: "action_panel",     svg: "layout-sidebar-right" },
];

// ---------------------------------------------------------------------------
// Material Design Icons (https://fonts.google.com/icons). Apache 2.0 License.
// Estructura: material-design-icons-master/src/<category>/<name>/materialicons/24px.svg
// NOTA: `prefix` se fija a "material-design-icons-master/src/" y `svg` contiene
// "<category>/<name>/materialicons/24px.svg" para reflejar la jerarquía real del zip.
// ---------------------------------------------------------------------------
const MATERIAL: &[Map] = &[
    Map { key: "folder",           svg: "file/folder/materialicons/24px.svg" },
    // device/storage = ícono de almacenamiento (cilindro de base de datos)
    Map { key: "drive",            svg: "device/storage/materialicons/24px.svg" },
    Map { key: "unknown",          svg: "action/help/materialicons/24px.svg" },
    Map { key: "file_image",       svg: "image/image/materialicons/24px.svg" },
    Map { key: "file_video",       svg: "av/video_file/materialicons/24px.svg" },
    Map { key: "file_audio",       svg: "av/audio_file/materialicons/24px.svg" },
    Map { key: "file_document",    svg: "action/description/materialicons/24px.svg" },
    Map { key: "file_code",        svg: "action/code/materialicons/24px.svg" },
    Map { key: "file_archive",     svg: "content/archive/materialicons/24px.svg" },
    // action/terminal es el ícono de consola/ejecutable más cercano
    Map { key: "file_executable",  svg: "action/terminal/materialicons/24px.svg" },
    // action/view_in_ar = modelo en realidad aumentada (3D)
    Map { key: "file_model3d",     svg: "action/view_in_ar/materialicons/24px.svg" },
    Map { key: "file_font",        svg: "content/font_download/materialicons/24px.svg" },
    // editor/insert_drive_file = archivo genérico
    Map { key: "file_generic",     svg: "editor/insert_drive_file/materialicons/24px.svg" },
    Map { key: "action_back",      svg: "navigation/arrow_back/materialicons/24px.svg" },
    Map { key: "action_forward",   svg: "navigation/arrow_forward/materialicons/24px.svg" },
    Map { key: "action_up",        svg: "navigation/arrow_upward/materialicons/24px.svg" },
    Map { key: "action_refresh",   svg: "navigation/refresh/materialicons/24px.svg" },
    Map { key: "action_copy",      svg: "content/content_copy/materialicons/24px.svg" },
    Map { key: "action_cut",       svg: "content/content_cut/materialicons/24px.svg" },
    Map { key: "action_paste",     svg: "content/content_paste/materialicons/24px.svg" },
    Map { key: "action_delete",    svg: "action/delete/materialicons/24px.svg" },
    // action/note_add = nuevo archivo (nota con +)
    Map { key: "action_new_file",  svg: "action/note_add/materialicons/24px.svg" },
    Map { key: "action_new_folder",svg: "file/create_new_folder/materialicons/24px.svg" },
    // action/view_sidebar = panel lateral (agregar panel)
    Map { key: "action_add_pane",  svg: "action/view_sidebar/materialicons/24px.svg" },
    // action/swap_horiz = intercambiar paneles horizontalmente
    Map { key: "action_swap_panes",svg: "action/swap_horiz/materialicons/24px.svg" },
    // content/file_copy = clonar/duplicar archivo o ruta
    Map { key: "action_clone_path",svg: "content/file_copy/materialicons/24px.svg" },
    // action/open_in_new = abrir en nueva ventana
    Map { key: "action_new_window",svg: "action/open_in_new/materialicons/24px.svg" },
    Map { key: "action_settings",  svg: "action/settings/materialicons/24px.svg" },
    // action/tab = pestaña individual (sin "tabs" en plural en material)
    Map { key: "action_tabs",      svg: "action/tab/materialicons/24px.svg" },
    // action/view_column = vista de columnas (gestión de layouts)
    Map { key: "action_layouts",   svg: "action/view_column/materialicons/24px.svg" },
    Map { key: "action_terminal",  svg: "action/terminal/materialicons/24px.svg" },
    // material tiene eject nativo (verificado en action/)
    Map { key: "action_eject",     svg: "action/eject/materialicons/24px.svg" },
    // action/view_quilt = vista de mosaico/panel
    Map { key: "action_panel",     svg: "action/view_quilt/materialicons/24px.svg" },
];

// ---------------------------------------------------------------------------
// Flat Color Icons (https://github.com/icons8/flat-color-icons). MIT License.
// Estructura: flat-color-icons-master/svg/<name>.svg
// NOTA: este set tiene color plano; tintable = false.
// ---------------------------------------------------------------------------
const FLAT_COLOR: &[Map] = &[
    Map { key: "folder",           svg: "folder" },
    // flat-color no tiene ícono de disco duro; display (monitor) es el sustituto más cercano
    Map { key: "drive",            svg: "display" },
    // flat-color no tiene ícono genérico de "desconocido"; file.svg es el sustituto
    Map { key: "unknown",          svg: "file" },
    Map { key: "file_image",       svg: "image_file" },
    Map { key: "file_video",       svg: "video_file" },
    Map { key: "file_audio",       svg: "audio_file" },
    Map { key: "file_document",    svg: "document" },
    // flat-color no tiene ícono de código; file.svg es el sustituto genérico
    Map { key: "file_code",        svg: "file" },
    // package.svg = caja empaquetada (archivo comprimido)
    Map { key: "file_archive",     svg: "package" },
    // flat-color no tiene ejecutable; command_line es el sustituto más técnico
    Map { key: "file_executable",  svg: "command_line" },
    // flat-color no tiene 3D; tree_structure es el sustituto estructural
    Map { key: "file_model3d",     svg: "tree_structure" },
    // flat-color no tiene fuentes tipográficas; news.svg (texto/papel) es el sustituto
    Map { key: "file_font",        svg: "news" },
    Map { key: "file_generic",     svg: "file" },
    // left.svg = flecha izquierda (volver atrás)
    Map { key: "action_back",      svg: "left" },
    // right.svg = flecha derecha (avanzar)
    Map { key: "action_forward",   svg: "right" },
    // up.svg = flecha arriba (subir un nivel)
    Map { key: "action_up",        svg: "up" },
    Map { key: "action_refresh",   svg: "refresh" },
    // copyleft.svg (C inversa) es el icono más parecido a "copiar" en flat-color
    Map { key: "action_copy",      svg: "copyleft" },
    // flat-color no tiene scissors/cut; previous.svg (retroceder) es el sustituto aproximado
    Map { key: "action_cut",       svg: "previous" },
    // flat-color no tiene clipboard/paste; import.svg es el sustituto de "pegar desde afuera"
    Map { key: "action_paste",     svg: "import" },
    // empty_trash = eliminar (papelera vacía lista para usar)
    Map { key: "action_delete",    svg: "empty_trash" },
    // flat-color no tiene new-file; plus.svg es el genérico de "agregar"
    Map { key: "action_new_file",  svg: "plus" },
    // opened_folder = carpeta abierta (abrir/crear carpeta nueva)
    Map { key: "action_new_folder",svg: "opened_folder" },
    // flat-color no tiene iconos de panel; internal.svg (dividir internamente) es el sustituto
    Map { key: "action_add_pane",  svg: "internal" },
    // flat-color no tiene swap; currency_exchange (intercambio) es el sustituto visual
    Map { key: "action_swap_panes",svg: "currency_exchange" },
    // flat-color no tiene clonar ruta; external.svg (flecha afuera) es el sustituto
    Map { key: "action_clone_path",svg: "external" },
    // flat-color no tiene new-window; advance.svg (ventana con flecha) es el sustituto
    Map { key: "action_new_window",svg: "advance" },
    Map { key: "action_settings",  svg: "settings" },
    // flat-color no tiene tabs; smartphone_tablet.svg (múltiples pantallas) es el sustituto
    Map { key: "action_tabs",      svg: "smartphone_tablet" },
    // flat-color no tiene layouts; serial_tasks.svg (columnas en serie) es el sustituto
    Map { key: "action_layouts",   svg: "serial_tasks" },
    // command_line.svg = terminal (consola de comandos)
    Map { key: "action_terminal",  svg: "command_line" },
    // flat-color no tiene eject; usb.svg (dispositivo extraíble USB) es el sustituto
    Map { key: "action_eject",     svg: "usb" },
    // flat-color no tiene panel; grid.svg (cuadrícula de paneles) es el sustituto
    Map { key: "action_panel",     svg: "grid" },
];

// ---------------------------------------------------------------------------
// Mono — reutiliza los mismos SVG de Lucide; la diferencia (tinte gris) se
// aplica en la rasterización (Task 6). gray = true lo indica al generador.
// ---------------------------------------------------------------------------
const MONO: &[Map] = LUCIDE;

// ---------------------------------------------------------------------------
// Declaración de los 5 sets
// ---------------------------------------------------------------------------

fn all_specs() -> [SetSpec; 5] {
    [
        SetSpec {
            id: "lucide",
            zip: "assets/icons/lucide-main.zip",
            prefix: "lucide-main/icons/",
            tintable: true,
            gray: false,
            maps: LUCIDE,
        },
        SetSpec {
            id: "tabler",
            zip: "assets/icons/tabler-icons-main.zip",
            prefix: "tabler-icons-main/icons/outline/",
            tintable: true,
            gray: false,
            maps: TABLER,
        },
        SetSpec {
            id: "material",
            zip: "assets/icons/material-design-icons-master.zip",
            // El campo `svg` en MATERIAL ya incluye <category>/<name>/materialicons/24px.svg
            prefix: "material-design-icons-master/src/",
            tintable: true,
            gray: false,
            maps: MATERIAL,
        },
        SetSpec {
            id: "flat-color",
            zip: "assets/icons/flat-color-icons-master.zip",
            prefix: "flat-color-icons-master/svg/",
            tintable: false,
            gray: false,
            maps: FLAT_COLOR,
        },
        SetSpec {
            id: "mono",
            zip: "assets/icons/lucide-main.zip",
            prefix: "lucide-main/icons/",
            tintable: true,
            gray: true,
            maps: MONO,
        },
    ]
}

// ---------------------------------------------------------------------------
// main — stub hasta Task 6 donde se implementa la rasterización SVG -> PNG
// ---------------------------------------------------------------------------
fn main() {
    let specs = all_specs();
    eprintln!("gen_icons: {} sets configurados (rasterización implementada en Task 6)", specs.len());
    for s in &specs {
        eprintln!(
            "  set={:12} zip={} entradas={}",
            s.id,
            s.zip,
            s.maps.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Tests de cobertura: cada set debe mapear exactamente las 33 claves canónicas
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    const ALL_KEYS: &[&str] = &[
        "folder",
        "drive",
        "unknown",
        "file_image",
        "file_video",
        "file_audio",
        "file_document",
        "file_code",
        "file_archive",
        "file_executable",
        "file_model3d",
        "file_font",
        "file_generic",
        "action_back",
        "action_forward",
        "action_up",
        "action_refresh",
        "action_copy",
        "action_cut",
        "action_paste",
        "action_delete",
        "action_new_file",
        "action_new_folder",
        "action_add_pane",
        "action_swap_panes",
        "action_clone_path",
        "action_new_window",
        "action_settings",
        "action_tabs",
        "action_layouts",
        "action_terminal",
        "action_eject",
        "action_panel",
    ];

    fn assert_cubre(maps: &[Map], set: &str) {
        for k in ALL_KEYS {
            assert!(
                maps.iter().any(|m| m.key == *k),
                "{set}: falta mapeo de {k}"
            );
        }
        assert_eq!(
            maps.len(),
            ALL_KEYS.len(),
            "{set}: sobran/faltan mapeos (esperado={}, actual={})",
            ALL_KEYS.len(),
            maps.len()
        );
    }

    #[test]
    fn lucide_cubre_todas_las_claves() {
        assert_cubre(LUCIDE, "lucide");
    }

    #[test]
    fn tabler_cubre_todas_las_claves() {
        assert_cubre(TABLER, "tabler");
    }

    #[test]
    fn material_cubre_todas_las_claves() {
        assert_cubre(MATERIAL, "material");
    }

    #[test]
    fn flat_color_cubre_todas_las_claves() {
        assert_cubre(FLAT_COLOR, "flat-color");
    }

    #[test]
    fn mono_cubre_todas_las_claves() {
        assert_cubre(MONO, "mono");
    }
}
