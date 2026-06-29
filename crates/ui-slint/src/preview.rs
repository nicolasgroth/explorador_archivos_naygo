// Naygo — preview liviano para la UI Slint: worker con debounce + cancelación.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Espeja el preview de la capa egui: al enfocar un archivo se espera un debounce (150 ms)
// y recién entonces un worker lee/decodifica en su hilo. El worker produce un `Payload`
// con bytes crudos (texto ya truncado, o RGBA ya escalada): NUNCA construye tipos de Slint
// (eso ocurre en el hilo de UI, que arma el `slint::Image`). Cada worker es cancelable: si
// el foco cambia, el token se cancela y el resultado tardío se descarta por path.

use naygo_core::cancel::CancellationToken;
use naygo_core::highlight::HlLine;
use naygo_core::preview::{self, CodeLang, PreviewKind, PreviewRule};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// Debounce: mover rápido por una carpeta NO dispara una lectura por archivo.
pub const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(150);

/// Última vista entregada, cacheada para pintar en cada tick sin reconstruir el worker
/// (evita parpadeo entre el momento en que llega el resultado y el siguiente foco). Guarda
/// datos crudos; el `slint::Image` se arma en el hilo de UI desde `Image.rgba`.
#[derive(Clone, Debug)]
pub enum ViewCache {
    Text {
        text: String,
        truncated: bool,
        /// Líneas ya resaltadas si la regla forzó un lenguaje de código; `None` = texto plano.
        highlighted: Option<Vec<HlLine>>,
    },
    Image {
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    Message(String),
}

/// Resultado crudo del worker (sin tipos de Slint). El hilo de UI lo convierte a `PreviewVm`.
#[derive(Clone, Debug)]
pub enum Payload {
    /// Texto truncado listo para pintar (+ si se cortó, para el aviso). `highlighted` lleva las
    /// líneas resaltadas cuando la regla forzó un lenguaje de código (sino `None` = texto plano).
    Text {
        text: String,
        truncated: bool,
        highlighted: Option<Vec<HlLine>>,
    },
    /// Imagen decodificada: RGBA8 + dimensiones (ya reescalada al tope).
    Image {
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// No previsualizable / muy grande / error: la UI muestra el mensaje.
    Message(String),
}

/// Estado del preview: qué se quiere, desde cuándo (debounce), qué está cargado, el worker
/// en vuelo y su token. Es propiedad del controlador.
pub struct PreviewState {
    /// Path enfocado a previsualizar (None = nada).
    pub wanted: Option<PathBuf>,
    /// Ancla del debounce desde el último cambio de `wanted`.
    since: Option<Instant>,
    /// Path cuyo `Payload` ya está cargado y entregado a la UI.
    pub loaded: Option<PathBuf>,
    /// Worker en vuelo (envía una vez y termina).
    rx: Option<Receiver<(PathBuf, Payload)>>,
    /// Token para cancelar el worker al cambiar el foco.
    token: Option<CancellationToken>,
    /// Reglas de clasificación (qué extensiones son texto/imagen).
    rules: Vec<PreviewRule>,
    /// Toggle global de auto-resaltado de código (`Settings.auto_highlight_code`). Si está en
    /// `false`, el worker pinta el código como texto plano aunque la regla fuerce un lenguaje.
    auto_highlight: bool,
    /// Última vista entregada (para pintar en cada tick). `None` = nada cargado aún.
    last: Option<ViewCache>,
}

impl Default for PreviewState {
    fn default() -> Self {
        PreviewState {
            wanted: None,
            since: None,
            loaded: None,
            rx: None,
            token: None,
            rules: preview::default_preview_rules(),
            auto_highlight: true,
            last: None,
        }
    }
}

impl PreviewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fija el archivo enfocado (None = ninguno). Si cambió, reinicia el debounce y cancela
    /// el worker anterior. Devuelve true si cambió el objetivo.
    pub fn set_wanted(&mut self, file: Option<PathBuf>, now: Instant) -> bool {
        if self.wanted == file {
            return false;
        }
        self.wanted = file.clone();
        self.since = Some(now);
        if let Some(t) = self.token.take() {
            t.cancel();
        }
        self.rx = None;
        if file.is_none() {
            self.loaded = None;
            self.last = None;
        }
        true
    }

    /// Refleja el toggle global `Settings.auto_highlight_code`. El controlador lo sincroniza
    /// antes de lanzar el worker, así el siguiente preview respeta la preferencia actual.
    pub fn set_auto_highlight(&mut self, on: bool) {
        self.auto_highlight = on;
    }

    /// La última vista entregada, para pintar (None = nada cargado).
    pub fn last_view(&self) -> Option<&ViewCache> {
        self.last.as_ref()
    }

    /// ¿Hay que arrancar el worker AHORA? (hay objetivo distinto del cargado, no hay worker
    /// en vuelo, y venció el debounce). `now` es el instante actual.
    pub fn should_start(&self, now: Instant) -> bool {
        let needs = match (&self.wanted, &self.loaded) {
            (Some(w), loaded) => Some(w) != loaded.as_ref() && self.rx.is_none(),
            (None, _) => false,
        };
        if !needs {
            return false;
        }
        self.since
            .map(|t| now.duration_since(t) >= PREVIEW_DEBOUNCE)
            .unwrap_or(true)
    }

    /// Lanza el worker para el `wanted` actual (asume `should_start` == true).
    pub fn start(&mut self) {
        let Some(path) = self.wanted.clone() else {
            return;
        };
        let rules = self.rules.clone();
        let auto_highlight = self.auto_highlight;
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let payload = build_payload(&path, &rules, auto_highlight, &worker_token);
            let _ = tx.send((path, payload));
        });
        self.token = Some(token);
        self.rx = Some(rx);
    }

    /// Drena el worker (sin bloquear). Si llegó un resultado para el path aún enfocado, lo
    /// devuelve y marca `loaded`; un resultado obsoleto se descarta. None si nada listo.
    pub fn poll(&mut self) -> Option<Payload> {
        let rx = self.rx.as_ref()?;
        match rx.try_recv() {
            Ok((path, payload)) => {
                self.rx = None;
                self.token = None;
                if Some(&path) == self.wanted.as_ref() {
                    self.loaded = Some(path);
                    self.last = Some(match &payload {
                        Payload::Text {
                            text,
                            truncated,
                            highlighted,
                        } => ViewCache::Text {
                            text: text.clone(),
                            truncated: *truncated,
                            highlighted: highlighted.clone(),
                        },
                        Payload::Image {
                            rgba,
                            width,
                            height,
                        } => ViewCache::Image {
                            rgba: rgba.clone(),
                            width: *width,
                            height: *height,
                        },
                        Payload::Message(m) => ViewCache::Message(m.clone()),
                    });
                    Some(payload)
                } else {
                    None
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.rx = None;
                self.token = None;
                None
            }
        }
    }

    /// ¿Hay trabajo pendiente (worker en vuelo o debounce sin vencer)? Para que el timer
    /// siga vivo hasta entregar el preview.
    pub fn busy(&self) -> bool {
        self.rx.is_some()
            || match (&self.wanted, &self.loaded) {
                (Some(w), loaded) => Some(w) != loaded.as_ref(),
                _ => false,
            }
    }
}

/// Construye el payload leyendo/decodificando en el hilo del worker.
fn build_payload(
    path: &Path,
    rules: &[PreviewRule],
    auto_highlight: bool,
    token: &CancellationToken,
) -> Payload {
    // Los .zip muestran su contenido (lista de entradas), antes de la clasificación normal.
    if is_zip(path) {
        return read_zip_listing(path);
    }
    match preview::classify_rules(path, rules) {
        PreviewKind::None => Payload::Message("No previsualizable".to_string()),
        // Si la regla fuerza un lenguaje y el toggle global está activo, `code_lang_for` lo
        // devuelve y el texto se resalta; con el toggle en `false` cae a texto plano.
        PreviewKind::Text => read_text(path, preview::code_lang_for(path, rules, auto_highlight)),
        PreviewKind::Image => read_image(path, token),
        PreviewKind::Svg => read_svg(path, token),
        PreviewKind::Pdf => read_pdf(path),
    }
}

/// `true` si la extensión (case-insensitive) es `zip`.
fn is_zip(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

/// Cuántas entradas del zip listar como máximo (evita textos enormes en archivos con miles).
const ZIP_MAX_ENTRIES: usize = 500;

/// Lee la lista de archivos de un .zip y la devuelve como texto (una entrada por línea, con su
/// tamaño descomprimido). Las carpetas se marcan con `/` final. No extrae contenido: solo el
/// índice central del zip, así que es liviano.
fn read_zip_listing(path: &Path) -> Payload {
    let Ok(file) = std::fs::File::open(path) else {
        return Payload::Message("No se pudo leer".to_string());
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return Payload::Message("ZIP inválido o dañado".to_string());
    };
    let total = archive.len();
    let mut lines: Vec<String> = Vec::with_capacity(total.min(ZIP_MAX_ENTRIES) + 2);
    lines.push(format!("{total} elemento(s) en el archivo:"));
    lines.push(String::new());
    let shown = total.min(ZIP_MAX_ENTRIES);
    for i in 0..shown {
        let Ok(entry) = archive.by_index(i) else {
            continue;
        };
        let name = entry.name().to_string();
        if entry.is_dir() {
            lines.push(format!("  {name}"));
        } else {
            lines.push(format!(
                "  {name}  ({})",
                naygo_core::format::format_size(entry.size(), naygo_core::format::SizeFormat::Auto)
            ));
        }
    }
    let truncated = total > shown;
    if truncated {
        lines.push(String::new());
        lines.push(format!("… y {} más", total - shown));
    }
    Payload::Text {
        text: lines.join("\n"),
        truncated,
        highlighted: None,
    }
}

/// Lee el archivo como texto y, si `lang` está presente (la regla fuerza un lenguaje), resalta
/// el texto YA recortado con `core::highlight` (defensa i16 intacta: se resalta lo recortado).
fn read_text(path: &Path, lang: Option<CodeLang>) -> Payload {
    use naygo_core::preview::{truncate_text, TEXT_MAX_BYTES};
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return Payload::Message("No se pudo leer".to_string());
    };
    let mut buf = Vec::with_capacity(TEXT_MAX_BYTES.min(8192));
    let mut chunk = [0u8; 8192];
    let mut hit_cap = false;
    loop {
        match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() >= TEXT_MAX_BYTES {
                    buf.truncate(TEXT_MAX_BYTES);
                    hit_cap = true;
                    break;
                }
            }
            Err(_) => return Payload::Message("No se pudo leer".to_string()),
        }
    }
    let t = truncate_text(&buf, hit_cap);
    // El resaltado opera SOBRE el texto ya recortado por `truncate_text` (≤ TEXT_MAX_LINES y
    // ≤ TEXT_MAX_LINE_CHARS por línea), así que ningún glifo cae fuera del rango i16.
    let highlighted = lang.map(|l| naygo_core::highlight::highlight(&t.text, l));
    Payload::Text {
        text: t.text,
        truncated: t.truncated,
        highlighted,
    }
}

fn read_image(path: &Path, token: &CancellationToken) -> Payload {
    use naygo_core::preview::{IMAGE_MAX_BYTES, IMAGE_MAX_SIDE};
    match std::fs::metadata(path) {
        Ok(m) if m.len() > IMAGE_MAX_BYTES => {
            return Payload::Message("Imagen muy grande".to_string())
        }
        Ok(_) => {}
        Err(_) => return Payload::Message("No se pudo leer".to_string()),
    }
    if token.is_cancelled() {
        return Payload::Message("Cancelado".to_string());
    }
    let Ok(img) = image::open(path) else {
        return Payload::Message("No se pudo decodificar".to_string());
    };
    if token.is_cancelled() {
        return Payload::Message("Cancelado".to_string());
    }
    let (w, h) = (img.width(), img.height());
    let scaled = if w > IMAGE_MAX_SIDE || h > IMAGE_MAX_SIDE {
        img.thumbnail(IMAGE_MAX_SIDE, IMAGE_MAX_SIDE)
    } else {
        img
    };
    let rgba = scaled.to_rgba8();
    Payload::Image {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    }
}

/// Rasteriza un SVG a RGBA con resvg (puro Rust, sin DLLs). Lo escala para que el lado mayor
/// quede en ~`IMAGE_MAX_SIDE` px (nítido pero acotado). Como el SVG es vectorial, se respeta el
/// tope de bytes del ARCHIVO fuente (no del bitmap resultante). Cancelable entre etapas.
fn read_svg(path: &Path, token: &CancellationToken) -> Payload {
    use naygo_core::preview::{IMAGE_MAX_BYTES, IMAGE_MAX_SIDE};
    let bytes = match std::fs::metadata(path) {
        Ok(m) if m.len() > IMAGE_MAX_BYTES => {
            return Payload::Message("SVG muy grande".to_string())
        }
        Ok(_) => match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return Payload::Message("No se pudo leer".to_string()),
        },
        Err(_) => return Payload::Message("No se pudo leer".to_string()),
    };
    if token.is_cancelled() {
        return Payload::Message("Cancelado".to_string());
    }
    // usvg parsea el SVG a un árbol simplificado. Opciones por defecto (sin fuentes externas:
    // el texto del SVG usa la BD de fuentes por defecto, suficiente para un preview).
    let opt = usvg::Options::default();
    let tree = match usvg::Tree::from_data(&bytes, &opt) {
        Ok(t) => t,
        Err(_) => return Payload::Message("SVG inválido".to_string()),
    };
    if token.is_cancelled() {
        return Payload::Message("Cancelado".to_string());
    }
    // Escala para encajar el lado mayor en IMAGE_MAX_SIDE (sin agrandar SVGs ya pequeños).
    let size = tree.size();
    let (sw, sh) = (size.width(), size.height());
    let longest = sw.max(sh).max(1.0);
    let scale = (IMAGE_MAX_SIDE as f32 / longest).clamp(0.01, 1.0);
    let pw = (sw * scale).ceil().max(1.0) as u32;
    let ph = (sh * scale).ceil().max(1.0) as u32;
    let mut pixmap = match tiny_skia::Pixmap::new(pw, ph) {
        Some(p) => p,
        None => return Payload::Message("No se pudo rasterizar".to_string()),
    };
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    if token.is_cancelled() {
        return Payload::Message("Cancelado".to_string());
    }
    // El pixmap de tiny-skia ya es RGBA8 (alpha premultiplicado). Para el preview sobre el panel
    // es aceptable mostrarlo tal cual; lo des-premultiplicamos a RGBA recto para que los bordes
    // semitransparentes no se vean oscurecidos.
    let rgba = unpremultiply_rgba(pixmap.data(), pw, ph);
    Payload::Image {
        width: pw,
        height: ph,
        rgba,
    }
}

/// Convierte RGBA premultiplicado (tiny-skia) a RGBA recto (lo que espera el panel). Divide cada
/// canal de color por el alpha. Píxeles totalmente transparentes quedan en 0.
fn unpremultiply_rgba(data: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for px in data.chunks_exact(4) {
        let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let unp = |c: u8| ((c as u16 * 255 + a as u16 / 2) / a as u16).min(255) as u8;
            out.extend_from_slice(&[unp(r), unp(g), unp(b), a]);
        }
    }
    out
}

/// Cuántos caracteres de texto del PDF mostrar como máximo (preview liviano).
const PDF_TEXT_MAX_CHARS: usize = 8000;
/// Tope de bytes del PDF a abrir (archivos enormes se evitan).
const PDF_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// Preview LIVIANO de un PDF: extrae el texto (puro Rust, sin DLLs) y un encabezado con el nº de
/// páginas. NO renderiza la página (eso requeriría una DLL nativa). Devuelve `Payload::Text`.
fn read_pdf(path: &Path) -> Payload {
    match std::fs::metadata(path) {
        Ok(m) if m.len() > PDF_MAX_BYTES => return Payload::Message("PDF muy grande".to_string()),
        Ok(_) => {}
        Err(_) => return Payload::Message("No se pudo leer".to_string()),
    }
    // Nº de páginas vía lopdf (barato: solo recorre el árbol de páginas).
    let pages = lopdf::Document::load(path)
        .ok()
        .map(|d| d.get_pages().len());
    // Texto vía pdf-extract (puede fallar en PDFs escaneados/protegidos → se avisa).
    let text = match pdf_extract::extract_text(path) {
        Ok(t) => t,
        Err(_) => {
            let head = match pages {
                Some(n) => format!("PDF de {n} página(s).\n\n"),
                None => String::new(),
            };
            return Payload::Text {
                text: format!("{head}(No se pudo extraer el texto: puede ser un PDF escaneado, protegido o con solo imágenes.)"),
                truncated: false,
                highlighted: None,
            };
        }
    };
    let header = match pages {
        Some(n) => format!("PDF de {n} página(s):\n\n"),
        None => "PDF:\n\n".to_string(),
    };
    let trimmed = text.trim();
    let too_long = trimmed.chars().count() > PDF_TEXT_MAX_CHARS;
    let capped: String = trimmed.chars().take(PDF_TEXT_MAX_CHARS).collect();
    // Recorte por línea: el texto extraído de un PDF puede traer líneas larguísimas que
    // desbordarían el render por software (glifo fuera del rango i16). Misma defensa que el
    // preview de texto.
    let (body, clipped) = naygo_core::preview::clip_long_lines(&capped);
    Payload::Text {
        text: format!("{header}{body}"),
        truncated: too_long || clipped,
        highlighted: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debounce_no_arranca_antes_del_plazo() {
        let mut s = PreviewState::new();
        let t0 = Instant::now();
        assert!(s.set_wanted(Some(PathBuf::from("C:/x/a.txt")), t0));
        // Justo al fijar, el debounce no venció.
        assert!(!s.should_start(t0));
        // Pasado el plazo, sí debe arrancar.
        assert!(s.should_start(t0 + PREVIEW_DEBOUNCE));
    }

    #[test]
    fn fijar_el_mismo_path_no_reinicia() {
        let mut s = PreviewState::new();
        let t0 = Instant::now();
        assert!(s.set_wanted(Some(PathBuf::from("C:/x/a.txt")), t0));
        assert!(!s.set_wanted(Some(PathBuf::from("C:/x/a.txt")), t0 + PREVIEW_DEBOUNCE));
    }

    #[test]
    fn sin_objetivo_no_hay_trabajo() {
        let mut s = PreviewState::new();
        let t0 = Instant::now();
        s.set_wanted(None, t0);
        assert!(!s.busy());
        assert!(!s.should_start(t0 + PREVIEW_DEBOUNCE));
    }

    #[test]
    fn zip_lista_entradas() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("paquete.zip");
        {
            let f = std::fs::File::create(&p).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
            zw.start_file("readme.txt", opts).unwrap();
            zw.write_all(b"hola mundo").unwrap();
            zw.start_file("src/main.rs", opts).unwrap();
            zw.write_all(b"fn main() {}").unwrap();
            zw.finish().unwrap();
        }
        match read_zip_listing(&p) {
            Payload::Text { text, .. } => {
                assert!(text.contains("readme.txt"), "lista readme: {text}");
                assert!(text.contains("src/main.rs"), "lista main: {text}");
                assert!(text.contains("2 elemento"), "cuenta entradas: {text}");
            }
            other => panic!("esperaba texto, fue {other:?}"),
        }
    }

    #[test]
    fn zip_invalido_da_mensaje() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("roto.zip");
        std::fs::write(&p, b"esto no es un zip").unwrap();
        match read_zip_listing(&p) {
            Payload::Message(_) => {}
            other => panic!("esperaba mensaje, fue {other:?}"),
        }
    }

    #[test]
    fn texto_se_lee_y_trunca() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("hola.txt");
        std::fs::write(&p, b"linea1\nlinea2\n").unwrap();
        let rules = preview::default_preview_rules();
        let token = CancellationToken::new();
        match build_payload(&p, &rules, true, &token) {
            Payload::Text {
                text, highlighted, ..
            } => {
                assert!(text.contains("linea1"));
                // Sin regla de código (`.txt` es Auto) → texto plano, sin resaltado.
                assert!(highlighted.is_none(), "txt en Auto no debe resaltar");
            }
            other => panic!("esperaba texto, fue {other:?}"),
        }
    }

    #[test]
    fn regla_de_codigo_resalta_el_texto() {
        // Una regla que fuerza `.dat` a código JSON → el worker resalta el texto leído.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.dat");
        std::fs::write(&p, b"{\n  \"a\": 1\n}\n").unwrap();
        let rules = vec![preview::PreviewRule {
            ext: "dat".to_string(),
            enabled: true,
            view: preview::ViewMode::Code(CodeLang::Json),
        }];
        let token = CancellationToken::new();
        // Con el toggle activo, la regla de código produce líneas resaltadas.
        match build_payload(&p, &rules, true, &token) {
            Payload::Text { highlighted, .. } => {
                let lines = highlighted.expect("debe traer líneas resaltadas");
                assert!(!lines.is_empty(), "el resaltado produce líneas");
            }
            other => panic!("esperaba texto, fue {other:?}"),
        }
    }

    #[test]
    fn toggle_controla_el_resaltado_automatico() {
        // Extensión de código conocida (`.rs`) SIN regla que fuerce lenguaje: el resaltado
        // depende del toggle. Con él activo se deduce el lenguaje y se resalta; apagado, no.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("lib.rs");
        std::fs::write(&p, b"fn main() {}\n").unwrap();
        let rules = preview::default_preview_rules();
        let token = CancellationToken::new();

        match build_payload(&p, &rules, true, &token) {
            Payload::Text { highlighted, .. } => {
                assert!(
                    highlighted.is_some(),
                    "con el toggle activo el código se resalta"
                );
            }
            other => panic!("esperaba texto, fue {other:?}"),
        }

        match build_payload(&p, &rules, false, &token) {
            Payload::Text { highlighted, .. } => {
                assert!(
                    highlighted.is_none(),
                    "con el toggle apagado no debe resaltar"
                );
            }
            other => panic!("esperaba texto, fue {other:?}"),
        }
    }
}
