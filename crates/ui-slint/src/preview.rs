// Naygo — preview liviano para la UI Slint: worker con debounce + cancelación.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Espeja el preview de la capa egui: al enfocar un archivo se espera un debounce (150 ms)
// y recién entonces un worker lee/decodifica en su hilo. El worker produce un `Payload`
// con bytes crudos (texto ya truncado, o RGBA ya escalada): NUNCA construye tipos de Slint
// (eso ocurre en el hilo de UI, que arma el `slint::Image`). Cada worker es cancelable: si
// el foco cambia, el token se cancela y el resultado tardío se descarta por path.

use naygo_core::cancel::CancellationToken;
use naygo_core::preview::{self, PreviewKind, PreviewRule};
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
    /// Texto truncado listo para pintar (+ si se cortó, para el aviso).
    Text { text: String, truncated: bool },
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
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let payload = build_payload(&path, &rules, &worker_token);
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
                        Payload::Text { text, truncated } => ViewCache::Text {
                            text: text.clone(),
                            truncated: *truncated,
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
fn build_payload(path: &Path, rules: &[PreviewRule], token: &CancellationToken) -> Payload {
    match preview::classify_rules(path, rules) {
        PreviewKind::None => Payload::Message("No previsualizable".to_string()),
        PreviewKind::Text => read_text(path),
        PreviewKind::Image => read_image(path, token),
    }
}

fn read_text(path: &Path) -> Payload {
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
    Payload::Text {
        text: t.text,
        truncated: t.truncated,
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
    fn texto_se_lee_y_trunca() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("hola.txt");
        std::fs::write(&p, b"linea1\nlinea2\n").unwrap();
        let rules = preview::default_preview_rules();
        let token = CancellationToken::new();
        match build_payload(&p, &rules, &token) {
            Payload::Text { text, .. } => assert!(text.contains("linea1")),
            other => panic!("esperaba texto, fue {other:?}"),
        }
    }
}
