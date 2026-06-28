// Naygo — generador build-time de los sets de íconos de fábrica (SVG -> PNG).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// No forma parte del binario de la app. Se ejecuta a mano para regenerar
// assets/icons/<set>/. Lee los SVG directamente de los .zip de las librerías y
// rasteriza cada IconKey a PNG (Task 6). Idempotente; nunca toca el config del usuario.

/// Una entrada de mapeo: clave estable de Naygo -> nombre del SVG dentro de la librería.
struct Map {
    key: &'static str,
    svg: &'static str,
}

/// Especificación de un set: id, zip fuente, prefijo dentro del zip, tintable, mapeo.
struct SetSpec {
    id: &'static str,
    zip: &'static str,    // ruta relativa al zip, p.ej. "assets/icons/lucide-main.zip"
    prefix: &'static str, // prefijo dentro del zip, p.ej. "lucide-main/icons/"
    tintable: bool,
    gray: bool, // true = forzar tinte gris (solo set mono); independiente de tintable
    maps: &'static [Map],
}

// ---------------------------------------------------------------------------
// Lucide (https://lucide.dev). MIT License.
// Estructura: lucide-main/icons/<name>.svg
// ---------------------------------------------------------------------------
const LUCIDE: &[Map] = &[
    Map { key: "folder",            svg: "folder" },
    Map { key: "drive",             svg: "hard-drive" },
    Map { key: "unknown",           svg: "file" },
    Map { key: "file_image",        svg: "file-image" },
    // lucide no tiene file-video.svg; el más cercano es file-video-camera
    Map { key: "file_video",        svg: "file-video-camera" },
    Map { key: "file_audio",        svg: "file-music" },
    Map { key: "file_document",     svg: "file-text" },
    Map { key: "file_code",         svg: "file-code" },
    Map { key: "file_archive",      svg: "file-archive" },
    // lucide no tiene file-executable; file-terminal es lo más representativo
    Map { key: "file_executable",   svg: "file-terminal" },
    // box.svg existe en lucide (verificado)
    Map { key: "file_model3d",      svg: "box" },
    // lucide no tiene file-font; type.svg (la letra T) es el sustituto tipográfico
    Map { key: "file_font",         svg: "type" },
    Map { key: "file_generic",      svg: "file" },
    Map { key: "action_back",       svg: "arrow-left" },
    Map { key: "action_forward",    svg: "arrow-right" },
    Map { key: "action_up",         svg: "arrow-up" },
    Map { key: "action_refresh",    svg: "refresh-cw" },
    Map { key: "action_copy",       svg: "copy" },
    Map { key: "action_cut",        svg: "scissors" },
    Map { key: "action_paste",      svg: "clipboard" },
    Map { key: "action_delete",     svg: "trash" },
    Map { key: "action_new_file",   svg: "file-plus" },
    Map { key: "action_new_folder", svg: "folder-plus" },
    // panel-left como "agregar panel" (añade columna izquierda)
    Map { key: "action_add_pane",   svg: "panel-left" },
    Map { key: "action_swap_panes", svg: "arrow-left-right" },
    // copy-plus = "clonar ruta / duplicar panel"
    Map { key: "action_clone_path", svg: "copy-plus" },
    Map { key: "action_new_window", svg: "app-window" },
    Map { key: "action_settings",   svg: "settings" },
    // lucide no tiene tabs.svg; file-stack (apilar archivos) es el sustituto
    Map { key: "action_tabs",       svg: "file-stack" },
    Map { key: "action_layouts",    svg: "layout-panel-left" },
    Map { key: "action_terminal",   svg: "terminal" },
    // lucide no tiene eject.svg; disc.svg es el CD/disco expulsable más cercano
    Map { key: "action_eject",      svg: "disc" },
    Map { key: "action_panel",      svg: "panel-right" },
];

// ---------------------------------------------------------------------------
// Tabler Icons (https://tabler.io/icons). MIT License.
// Estructura: tabler-icons-main/icons/outline/<name>.svg
// ---------------------------------------------------------------------------
const TABLER: &[Map] = &[
    Map { key: "folder",            svg: "folder" },
    // tabler no tiene hard-drive.svg; device-desktop (monitor de escritorio) es lo más cercano
    Map { key: "drive",             svg: "device-desktop" },
    Map { key: "unknown",           svg: "file-unknown" },
    // tabler no tiene file-image; photo.svg es el equivalente
    Map { key: "file_image",        svg: "photo" },
    Map { key: "file_video",        svg: "video" },
    Map { key: "file_audio",        svg: "file-music" },
    Map { key: "file_document",     svg: "file-text" },
    Map { key: "file_code",         svg: "file-code" },
    // tabler no tiene file-archive; file-zip es el equivalente comprimido
    Map { key: "file_archive",      svg: "file-zip" },
    // tabler no tiene file-executable ni file-terminal; file-code-2 es lo más técnico
    Map { key: "file_executable",   svg: "file-code-2" },
    // tabler tiene file-3d (verificado)
    Map { key: "file_model3d",      svg: "file-3d" },
    // tabler no tiene file-font; typography.svg (letra T con serifa) es el sustituto
    Map { key: "file_font",         svg: "typography" },
    Map { key: "file_generic",      svg: "file" },
    Map { key: "action_back",       svg: "arrow-left" },
    Map { key: "action_forward",    svg: "arrow-right" },
    Map { key: "action_up",         svg: "arrow-up" },
    Map { key: "action_refresh",    svg: "refresh" },
    Map { key: "action_copy",       svg: "copy" },
    Map { key: "action_cut",        svg: "scissors" },
    // tabler no tiene clipboard-paste; clipboard.svg es el sustituto genérico
    Map { key: "action_paste",      svg: "clipboard" },
    Map { key: "action_delete",     svg: "trash" },
    Map { key: "action_new_file",   svg: "file-plus" },
    Map { key: "action_new_folder", svg: "folder-plus" },
    // columns-2 = dos columnas (agregar panel lateral)
    Map { key: "action_add_pane",   svg: "columns-2" },
    // arrows-exchange = intercambiar paneles
    Map { key: "action_swap_panes", svg: "arrows-exchange" },
    // copy-plus = clonar ruta en panel nuevo
    Map { key: "action_clone_path", svg: "copy-plus" },
    Map { key: "action_new_window", svg: "app-window" },
    Map { key: "action_settings",   svg: "settings" },
    // tabler no tiene tabs.svg; layout-sidebar es el más cercano para gestión de paneles
    Map { key: "action_tabs",       svg: "layout-sidebar" },
    Map { key: "action_layouts",    svg: "layout-columns" },
    Map { key: "action_terminal",   svg: "terminal" },
    // tabler no tiene eject.svg; player-eject existe (verificado)
    Map { key: "action_eject",      svg: "player-eject" },
    // layout-sidebar-right = panel derecho visible
    Map { key: "action_panel",      svg: "layout-sidebar-right" },
];

// ---------------------------------------------------------------------------
// Material Design Icons (https://fonts.google.com/icons). Apache 2.0 License.
// Estructura: material-design-icons-master/src/<category>/<name>/materialicons/24px.svg
// NOTA: `prefix` se fija a "material-design-icons-master/src/" y `svg` contiene
// "<category>/<name>/materialicons/24px.svg" para reflejar la jerarquía real del zip.
// ---------------------------------------------------------------------------
const MATERIAL: &[Map] = &[
    Map { key: "folder",            svg: "file/folder/materialicons/24px.svg" },
    // device/storage = ícono de almacenamiento (cilindro de base de datos)
    Map { key: "drive",             svg: "device/storage/materialicons/24px.svg" },
    Map { key: "unknown",           svg: "action/help/materialicons/24px.svg" },
    Map { key: "file_image",        svg: "image/image/materialicons/24px.svg" },
    Map { key: "file_video",        svg: "av/video_file/materialicons/24px.svg" },
    Map { key: "file_audio",        svg: "av/audio_file/materialicons/24px.svg" },
    Map { key: "file_document",     svg: "action/description/materialicons/24px.svg" },
    Map { key: "file_code",         svg: "action/code/materialicons/24px.svg" },
    Map { key: "file_archive",      svg: "content/archive/materialicons/24px.svg" },
    // action/terminal es el ícono de consola/ejecutable más cercano
    Map { key: "file_executable",   svg: "action/terminal/materialicons/24px.svg" },
    // action/view_in_ar = modelo en realidad aumentada (3D)
    Map { key: "file_model3d",      svg: "action/view_in_ar/materialicons/24px.svg" },
    Map { key: "file_font",         svg: "content/font_download/materialicons/24px.svg" },
    // editor/insert_drive_file = archivo genérico
    Map { key: "file_generic",      svg: "editor/insert_drive_file/materialicons/24px.svg" },
    Map { key: "action_back",       svg: "navigation/arrow_back/materialicons/24px.svg" },
    Map { key: "action_forward",    svg: "navigation/arrow_forward/materialicons/24px.svg" },
    Map { key: "action_up",         svg: "navigation/arrow_upward/materialicons/24px.svg" },
    Map { key: "action_refresh",    svg: "navigation/refresh/materialicons/24px.svg" },
    Map { key: "action_copy",       svg: "content/content_copy/materialicons/24px.svg" },
    Map { key: "action_cut",        svg: "content/content_cut/materialicons/24px.svg" },
    Map { key: "action_paste",      svg: "content/content_paste/materialicons/24px.svg" },
    Map { key: "action_delete",     svg: "action/delete/materialicons/24px.svg" },
    // action/note_add = nuevo archivo (nota con +)
    Map { key: "action_new_file",   svg: "action/note_add/materialicons/24px.svg" },
    Map { key: "action_new_folder", svg: "file/create_new_folder/materialicons/24px.svg" },
    // action/view_sidebar = panel lateral (agregar panel)
    Map { key: "action_add_pane",   svg: "action/view_sidebar/materialicons/24px.svg" },
    // action/swap_horiz = intercambiar paneles horizontalmente
    Map { key: "action_swap_panes", svg: "action/swap_horiz/materialicons/24px.svg" },
    // content/file_copy = clonar/duplicar archivo o ruta
    Map { key: "action_clone_path", svg: "content/file_copy/materialicons/24px.svg" },
    // action/open_in_new = abrir en nueva ventana
    Map { key: "action_new_window", svg: "action/open_in_new/materialicons/24px.svg" },
    Map { key: "action_settings",   svg: "action/settings/materialicons/24px.svg" },
    // action/tab = pestaña individual (sin "tabs" en plural en material)
    Map { key: "action_tabs",       svg: "action/tab/materialicons/24px.svg" },
    // action/view_column = vista de columnas (gestión de layouts)
    Map { key: "action_layouts",    svg: "action/view_column/materialicons/24px.svg" },
    Map { key: "action_terminal",   svg: "action/terminal/materialicons/24px.svg" },
    // material tiene eject nativo (verificado en action/)
    Map { key: "action_eject",      svg: "action/eject/materialicons/24px.svg" },
    // action/view_quilt = vista de mosaico/panel
    Map { key: "action_panel",      svg: "action/view_quilt/materialicons/24px.svg" },
];

// ---------------------------------------------------------------------------
// Flat Color Icons (https://github.com/icons8/flat-color-icons). MIT License.
// Estructura: flat-color-icons-master/svg/<name>.svg
// NOTA: este set tiene color plano; tintable = false.
// ---------------------------------------------------------------------------
const FLAT_COLOR: &[Map] = &[
    Map { key: "folder",            svg: "folder" },
    // flat-color no tiene ícono de disco duro; display (monitor) es el sustituto más cercano
    Map { key: "drive",             svg: "display" },
    // flat-color no tiene ícono genérico de "desconocido"; file.svg es el sustituto
    Map { key: "unknown",           svg: "file" },
    Map { key: "file_image",        svg: "image_file" },
    Map { key: "file_video",        svg: "video_file" },
    Map { key: "file_audio",        svg: "audio_file" },
    Map { key: "file_document",     svg: "document" },
    // flat-color no tiene ícono de código; file.svg es el sustituto genérico
    Map { key: "file_code",         svg: "file" },
    // package.svg = caja empaquetada (archivo comprimido)
    Map { key: "file_archive",      svg: "package" },
    // flat-color no tiene ejecutable; command_line es el sustituto más técnico
    Map { key: "file_executable",   svg: "command_line" },
    // flat-color no tiene 3D; tree_structure es el sustituto estructural
    Map { key: "file_model3d",      svg: "tree_structure" },
    // flat-color no tiene fuentes tipográficas; news.svg (texto/papel) es el sustituto
    Map { key: "file_font",         svg: "news" },
    Map { key: "file_generic",      svg: "file" },
    // left.svg = flecha izquierda (volver atrás)
    Map { key: "action_back",       svg: "left" },
    // right.svg = flecha derecha (avanzar)
    Map { key: "action_forward",    svg: "right" },
    // up.svg = flecha arriba (subir un nivel)
    Map { key: "action_up",         svg: "up" },
    Map { key: "action_refresh",    svg: "refresh" },
    // FIXME: flat-color no tiene icono de copiar (ni copy/duplicate/documents/paste);
    // copyleft (C invertida, de la familia copyright/copia) es lo más cercano disponible
    Map { key: "action_copy",       svg: "copyleft" },
    // FIXME: flat-color no tiene icono de cortar (ni scissors/cut); previous (flecha de
    // retroceso) es lo más cercano disponible — limitación del set, no un descuido
    Map { key: "action_cut",        svg: "previous" },
    // flat-color no tiene clipboard/paste; import.svg es el sustituto de "pegar desde afuera"
    Map { key: "action_paste",      svg: "import" },
    // empty_trash = eliminar (papelera vacía lista para usar)
    Map { key: "action_delete",     svg: "empty_trash" },
    // flat-color no tiene new-file; plus.svg es el genérico de "agregar"
    Map { key: "action_new_file",   svg: "plus" },
    // opened_folder = carpeta abierta (abrir/crear carpeta nueva)
    Map { key: "action_new_folder", svg: "opened_folder" },
    // flat-color no tiene iconos de panel; internal.svg (dividir internamente) es el sustituto
    Map { key: "action_add_pane",   svg: "internal" },
    // flat-color no tiene swap; currency_exchange (intercambio) es el sustituto visual
    Map { key: "action_swap_panes", svg: "currency_exchange" },
    // flat-color no tiene clonar ruta; external.svg (flecha afuera) es el sustituto
    Map { key: "action_clone_path", svg: "external" },
    // flat-color no tiene new-window; advance.svg (ventana con flecha) es el sustituto
    Map { key: "action_new_window", svg: "advance" },
    Map { key: "action_settings",   svg: "settings" },
    // flat-color no tiene tabs; smartphone_tablet.svg (múltiples pantallas) es el sustituto
    Map { key: "action_tabs",       svg: "smartphone_tablet" },
    // flat-color no tiene layouts; serial_tasks.svg (columnas en serie) es el sustituto
    Map { key: "action_layouts",    svg: "serial_tasks" },
    // command_line.svg = terminal (consola de comandos)
    Map { key: "action_terminal",   svg: "command_line" },
    // flat-color no tiene eject; usb.svg (dispositivo extraíble USB) es el sustituto
    Map { key: "action_eject",      svg: "usb" },
    // flat-color no tiene panel; grid.svg (cuadrícula de paneles) es el sustituto
    Map { key: "action_panel",      svg: "grid" },
];

// ---------------------------------------------------------------------------
// Mono — reutiliza los mismos SVG de Lucide; la diferencia (tinte gris) se
// aplica en la rasterización. gray = true lo indica al generador.
// ---------------------------------------------------------------------------
const MONO: &[Map] = LUCIDE;

// ---------------------------------------------------------------------------
// Declaración de los 5 sets
// ---------------------------------------------------------------------------

const fn all_specs() -> [SetSpec; 5] {
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
// Rasterización SVG -> PNG (solo disponible con feature gen-icons)
// ---------------------------------------------------------------------------
#[cfg(feature = "gen-icons")]
mod raster {
    use std::io::Read;
    use std::path::Path;
    use std::sync::LazyLock;

    use regex::Regex;

    use super::SetSpec;

    // Regex compilados una sola vez para todo el generador (recolor_svg se llama 165
    // veces; recompilar en cada llamada era innecesario).
    static RE_FILL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"fill="([^"]+)""#).unwrap());
    static RE_STROKE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"stroke="([^"]+)""#).unwrap());
    static RE_SVG_OPEN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(<svg\b[^>]*?)(/?>)").unwrap());

    /// Lee todas las entradas necesarias del zip en un único recorrido.
    /// Esto es crítico para zips muy grandes (p.ej. material-design-icons ~4GB):
    /// en vez de abrir el archivo y llamar `by_name` N veces, itera una sola vez
    /// y recoge solo las entradas que están en `needed`.
    ///
    /// Devuelve un mapa inner_path -> bytes para todas las entradas encontradas.
    /// Las entradas no encontradas se informan al llamador.
    pub fn read_svgs_from_zip(
        zip_path: &str,
        needed: &std::collections::HashSet<String>,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, String> {
        let file = std::fs::File::open(zip_path)
            .map_err(|e| format!("no se pudo abrir '{}': {}", zip_path, e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("zip inválido '{}': {}", zip_path, e))?;

        let mut result = std::collections::HashMap::new();
        let len = archive.len();
        for i in 0..len {
            let mut entry = archive.by_index(i).map_err(|e| {
                format!("error leyendo entrada #{} de '{}': {}", i, zip_path, e)
            })?;
            let name = entry.name().to_owned();
            if needed.contains(&name) {
                let mut bytes = Vec::with_capacity(entry.size() as usize);
                entry.read_to_end(&mut bytes).map_err(|e| {
                    format!("error leyendo '{}' de '{}': {}", name, zip_path, e)
                })?;
                result.insert(name, bytes);
                // Salir temprano si ya recogimos todo
                if result.len() == needed.len() {
                    break;
                }
            }
        }
        Ok(result)
    }

    /// Reemplaza todos los colores de fill/stroke en el SVG por `hex` (p.ej. "#FFFFFF").
    ///
    /// Estrategia en dos pasos:
    /// 1. Reemplaza currentColor, fill="<valor>" y stroke="<valor>" explícitos (Lucide/Tabler).
    /// 2. Inyecta `fill="<hex>"` en el elemento <svg> raíz para que los elementos sin fill
    ///    explícito hereden el color (Material Design: <path> sin atributos hereda fill=black).
    ///
    /// Los valores `none` y `url(...)` se preservan intactos.
    pub fn recolor_svg(svg: &str, hex: &str) -> String {
        // Paso 1: Reemplaza currentColor por el color destino
        let result = svg.replace("currentColor", hex);

        // Paso 1b: Reemplaza fill="<valor>" cuando el valor no es "none" ni empieza por "url("
        let result = RE_FILL.replace_all(&result, |caps: &regex::Captures| {
            let val = &caps[1];
            if val == "none" || val.starts_with("url(") {
                caps[0].to_string()
            } else {
                format!("fill=\"{}\"", hex)
            }
        });

        // Paso 1c: Reemplaza stroke="<valor>" cuando el valor no es "none" ni empieza por "url("
        let result = RE_STROKE.replace_all(&result, |caps: &regex::Captures| {
            let val = &caps[1];
            if val == "none" || val.starts_with("url(") {
                caps[0].to_string()
            } else {
                format!("stroke=\"{}\"", hex)
            }
        });

        // Paso 2: Inyectar fill en el elemento <svg> raíz si no tiene fill explícito.
        // Esto cubre Material Design (paths sin fill heredan del root en lugar del UA black).
        // Regex: <svg ... > o <svg ... />; inyecta fill="hex" antes del cierre del tag.
        // .replace (no replace_all): solo el <svg> raíz, no svg anidados.
        let result = RE_SVG_OPEN.replace(&result, |caps: &regex::Captures| {
            let tag_content = &caps[1];
            let close = &caps[2];
            // Solo inyectar si el <svg> raíz no tiene ya fill (para no duplicarlo)
            if tag_content.contains("fill=") {
                format!("{}{}", tag_content, close)
            } else {
                format!("{} fill=\"{}\"{}", tag_content, hex, close)
            }
        });

        result.into_owned()
    }

    /// Rasteriza `svg_bytes` a PNG de `size`×`size` píxeles.
    /// Si `tint_hex` es `Some(hex)`, recolorea el SVG antes de renderizar.
    ///
    /// El recoloreado (cuando `tint_hex` es `Some`) lo realiza `recolor_svg` a nivel
    /// de texto antes de parsear: reemplaza currentColor + fill/stroke explícitos e
    /// inyecta fill en el <svg> raíz para los paths sin fill (Material Design).
    pub fn rasterize(svg_bytes: &[u8], tint_hex: Option<&str>, size: u32) -> Result<Vec<u8>, String> {
        // Opcionalmente recolorea el SVG a texto y trabaja sobre esos bytes
        let recolored: Vec<u8>;
        let working_bytes: &[u8] = if let Some(hex) = tint_hex {
            let svg_str = std::str::from_utf8(svg_bytes)
                .map_err(|e| format!("SVG no es UTF-8 válido: {}", e))?;
            recolored = recolor_svg(svg_str, hex).into_bytes();
            &recolored
        } else {
            svg_bytes
        };

        // Parsear con usvg (el recoloreado textual ya aplicó el tinte en working_bytes)
        let opts = usvg::Options::default();
        let tree = usvg::Tree::from_data(working_bytes, &opts)
            .map_err(|e| format!("error parseando SVG: {}", e))?;

        // Calcular escala uniforme para caber en size×size
        let svg_w = tree.size().width();
        let svg_h = tree.size().height();
        let scale = if svg_w > 0.0 && svg_h > 0.0 {
            let sx = size as f32 / svg_w;
            let sy = size as f32 / svg_h;
            sx.min(sy)
        } else {
            1.0
        };
        let transform = tiny_skia::Transform::from_scale(scale, scale);

        // Crear pixmap y renderizar
        let mut pixmap = tiny_skia::Pixmap::new(size, size)
            .ok_or_else(|| format!("no se pudo crear pixmap {}x{}", size, size))?;
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        // Codificar a PNG
        pixmap
            .encode_png()
            .map_err(|e| format!("error codificando PNG: {}", e))
    }

    /// Escribe `<out_dir>/manifest.json` con los metadatos del set.
    pub fn write_manifest(out_dir: &Path, spec: &SetSpec) -> Result<(), String> {
        let label = match spec.id {
            "lucide"     => "Lucide",
            "tabler"     => "Tabler",
            "material"   => "Material",
            "flat-color" => "Flat Color",
            "mono"       => "Mono",
            other        => other,
        };
        let manifest = serde_json::json!({
            "id": spec.id,
            "label": label,
            "tintable": spec.tintable,
            "version": 1
        });
        let path = out_dir.join("manifest.json");
        std::fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap())
            .map_err(|e| format!("error escribiendo manifest en '{}': {}", path.display(), e))
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
#[cfg(feature = "gen-icons")]
fn main() {
    use std::collections::{HashMap, HashSet};
    use std::path::Path;
    use raster::{read_svgs_from_zip, rasterize, write_manifest};

    const SIZE: u32 = 48;
    let specs = all_specs();

    // mono comparte el zip de lucide (mismos SVG, tinte gris): lucide-main.zip se
    // lee dos veces a propósito. Es barato (~1MB) y mantiene los SetSpec simples.
    for spec in &specs {
        // Determinar color de tinte según el set
        let tint_hex: Option<&str> = if spec.gray {
            Some("#9E9E9E") // mono: gris
        } else if spec.tintable {
            Some("#FFFFFF") // lucide/tabler/material: blanco (la app tinta luego)
        } else {
            None // flat-color: renderizar en color
        };

        // Directorio de salida: assets/icons/<id>/
        let out_dir = Path::new("assets/icons").join(spec.id);
        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            eprintln!("ERROR creando '{}': {}", out_dir.display(), e);
            std::process::exit(1);
        }

        // Construir el conjunto de rutas internas que necesitamos del zip
        let inner_paths: Vec<(String, String)> = spec
            .maps
            .iter()
            .map(|m| {
                let inner = if spec.id == "material" {
                    // Material: svg ya incluye la extensión .svg
                    format!("{}{}", spec.prefix, m.svg)
                } else {
                    // Lucide/Tabler/Mono/FlatColor: agregar extensión .svg
                    format!("{}{}.svg", spec.prefix, m.svg)
                };
                (m.key.to_string(), inner)
            })
            .collect();

        let needed: HashSet<String> = inner_paths.iter().map(|(_, p)| p.clone()).collect();

        // Leer todos los SVG en un único recorrido del zip (crítico para zips grandes)
        eprintln!("Leyendo zip de '{}' (esto puede tardar para zips grandes)…", spec.id);
        let svgs: HashMap<String, Vec<u8>> = match read_svgs_from_zip(spec.zip, &needed) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("ERROR abriendo zip de '{}': {}", spec.id, e);
                std::process::exit(1);
            }
        };

        let mut ok = 0usize;
        let mut fail = 0usize;

        for (key, inner_path) in &inner_paths {
            let svg_bytes = match svgs.get(inner_path) {
                Some(b) => b,
                None => {
                    eprintln!(
                        "ERROR [{}] clave='{}' inner='{}': no encontrado en el zip",
                        spec.id, key, inner_path
                    );
                    fail += 1;
                    continue;
                }
            };

            // Rasterizar
            let png_bytes = match rasterize(svg_bytes, tint_hex, SIZE) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!(
                        "ERROR rasterizando [{}] clave='{}': {}",
                        spec.id, key, e
                    );
                    fail += 1;
                    continue;
                }
            };

            // Escribir PNG
            let png_path = out_dir.join(format!("{}.png", key));
            if let Err(e) = std::fs::write(&png_path, &png_bytes) {
                eprintln!(
                    "ERROR escribiendo '{}': {}",
                    png_path.display(), e
                );
                fail += 1;
                continue;
            }

            ok += 1;
        }

        // Manifest del set
        if let Err(e) = write_manifest(&out_dir, spec) {
            eprintln!("ERROR manifest [{}]: {}", spec.id, e);
        }

        if fail == 0 {
            eprintln!("set '{}' generado ({} íconos)", spec.id, ok);
        } else {
            eprintln!(
                "set '{}': {} íconos OK, {} FALLIDOS",
                spec.id, ok, fail
            );
        }
    }
}

#[cfg(not(feature = "gen-icons"))]
fn main() {
    let specs = all_specs();
    eprintln!(
        "gen_icons: {} sets configurados. Compilar con --features gen-icons para rasterizar.",
        specs.len()
    );
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
// Tests
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

    // Test de recolor_svg (solo con feature gen-icons)
    #[cfg(feature = "gen-icons")]
    mod recolor_tests {
        use super::super::raster::recolor_svg;

        #[test]
        fn recolor_stroke_currentcolor() {
            // SVG estilo Lucide/Tabler: usa stroke="currentColor"
            let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><line stroke="currentColor" stroke-width="2"/></svg>"#;
            let result = recolor_svg(svg, "#FFFFFF");
            assert!(
                result.contains("stroke=\"#FFFFFF\""),
                "debe contener stroke=\"#FFFFFF\", obtuvo: {result}"
            );
            assert!(
                !result.contains("currentColor"),
                "no debe quedar currentColor, obtuvo: {result}"
            );
        }

        #[test]
        fn recolor_fill_atributo() {
            // SVG estilo Material: usa fill="black"
            let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><path fill="black" d="M0 0"/></svg>"#;
            let result = recolor_svg(svg, "#9E9E9E");
            assert!(
                result.contains("fill=\"#9E9E9E\""),
                "debe contener fill=\"#9E9E9E\", obtuvo: {result}"
            );
        }

        #[test]
        fn recolor_preserva_none_y_url() {
            // fill="none" y stroke="url(#grad)" deben quedar intactos
            let svg = r#"<svg><path fill="none" stroke="url(#grad)"/></svg>"#;
            let result = recolor_svg(svg, "#FFFFFF");
            assert!(
                result.contains("fill=\"none\""),
                "fill=\"none\" debe preservarse, obtuvo: {result}"
            );
            assert!(
                result.contains("stroke=\"url(#grad)\""),
                "stroke url debe preservarse, obtuvo: {result}"
            );
        }

        #[test]
        fn recolor_material_path_sin_fill_explícito() {
            // SVG estilo Material: <path> sin fill explícito hereda del <svg> raíz.
            // La función debe inyectar fill en el <svg> raíz para que los paths hereden.
            let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" height="24" viewBox="0 0 24 24" width="24"><path d="M0 0h24v24H0z" fill="none"/><path d="M10 4H4c-1.1 0"/></svg>"#;
            let result = recolor_svg(svg, "#FFFFFF");
            // El <svg> raíz debe tener fill="#FFFFFF"
            assert!(
                result.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg""#)
                    && result.contains("fill=\"#FFFFFF\""),
                "el <svg> raíz debe tener fill=\"#FFFFFF\", obtuvo: {result}"
            );
            // El fill="none" del primer path debe preservarse
            assert!(
                result.contains("fill=\"none\""),
                "fill=\"none\" del path de fondo debe preservarse, obtuvo: {result}"
            );
        }
    }
}
