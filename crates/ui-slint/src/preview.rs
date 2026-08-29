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
use std::sync::{mpsc::Receiver, Arc};
use std::time::{Duration, Instant};

/// Debounce: mover rápido por una carpeta NO dispara una lectura por archivo.
pub const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(150);

/// Tras este tiempo sin contenido para el archivo enfocado, la UI ofrece cancelar. No es un
/// timeout: una red lenta o un archivo grande puede seguir; la decisión final siempre es del
/// usuario y el worker recibe el token cooperativo.
pub const PREVIEW_CANCEL_AFTER: Duration = Duration::from_secs(5);

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

/// Mensajes de error del preview, ya traducidos (los resuelve el hilo de UI, como ArchiveLabels).
/// El worker los usa directamente; así `Payload::Message` sigue siendo `String` y no hay
/// lógica de i18n en el hilo del worker.
#[derive(Clone, Debug)]
pub struct PreviewMessages {
    pub not_previewable: String,
    pub archive_bad: String,
    pub read: String,
    pub image_big: String,
    pub cancelled: String,
    pub decode: String,
    pub svg_big: String,
    pub svg_bad: String,
    pub rasterize: String,
    pub pdf_big: String,
    /// Encabezado del preview de PDF: frase con el placeholder `{n}` (nº de páginas), SIN
    /// puntuación final. El código añade `.` (cuando no se pudo extraer texto) o `:` (cuando
    /// sí) más los saltos, así una sola frase traducible cubre las dos variantes.
    pub pdf_pages: String,
    /// Aviso cuando no se pudo extraer texto de un PDF (escaneado/protegido/solo imágenes).
    pub pdf_no_text: String,
}

impl Default for PreviewMessages {
    fn default() -> Self {
        PreviewMessages {
            not_previewable: "No previsualizable".into(),
            archive_bad: "Archivo comprimido inválido o dañado".into(),
            read: "No se pudo leer".into(),
            image_big: "Imagen muy grande".into(),
            cancelled: "Cancelado".into(),
            decode: "No se pudo decodificar".into(),
            svg_big: "SVG muy grande".into(),
            svg_bad: "SVG inválido".into(),
            rasterize: "No se pudo rasterizar".into(),
            pdf_big: "PDF muy grande".into(),
            pdf_pages: "PDF de {n} página(s)".into(),
            pdf_no_text: "(No se pudo extraer el texto: puede ser un PDF escaneado, protegido o con solo imágenes.)".into(),
        }
    }
}

type MeshLoadMessage = (
    PathBuf,
    Result<Arc<naygo_core::mesh_preview::MeshScene>, naygo_core::mesh_preview::MeshError>,
);
type MeshRenderMessage = (
    PathBuf,
    Result<naygo_core::mesh_preview::MeshPreview, naygo_core::mesh_preview::MeshError>,
);

/// Estado del preview: qué se quiere, desde cuándo (debounce), qué está cargado, el worker
/// en vuelo y su token. Es propiedad del controlador.
pub struct PreviewState {
    /// Path enfocado a previsualizar (None = nada).
    pub wanted: Option<PathBuf>,
    /// Ancla del debounce desde el último cambio de `wanted`.
    since: Option<Instant>,
    /// Inicio del worker que todavía no entregó contenido para el archivo enfocado. Es distinto
    /// de `since`: el debounce no cuenta para los cinco segundos del botón Cancelar.
    loading_since: Option<Instant>,
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
    /// Etiquetas traducidas para el preview de comprimidos (las resuelve el hilo de UI con i18n).
    archive_labels: naygo_core::archive_tree::ArchiveLabels,
    /// Mensajes de error del preview, ya traducidos (los resuelve el hilo de UI con i18n).
    preview_msgs: PreviewMessages,
    /// Última vista entregada (para pintar en cada tick). `None` = nada cargado aún.
    last: Option<ViewCache>,
    /// Escena 3D cargada en memoria para que arrastrar el modelo no vuelva a leer STL/3MF desde
    /// disco. `Arc` permite pasarla al worker de rasterización sin copiar sus triángulos.
    mesh_scene: Option<(PathBuf, Arc<naygo_core::mesh_preview::MeshScene>)>,
    /// Worker que carga la geometría real al iniciar la primera interacción 3D.
    mesh_load_rx: Option<Receiver<MeshLoadMessage>>,
    /// Worker de rasterización de la escena ya cacheada.
    mesh_render_rx: Option<Receiver<MeshRenderMessage>>,
    /// Token compartido por la carga/rasterización 3D; se cancela al cambiar de archivo.
    mesh_token: Option<CancellationToken>,
    /// Cámara acumulada por arrastre/rueda. El último gesto reemplaza el render pendiente.
    mesh_camera: naygo_core::mesh_preview::MeshCamera,
}

impl Default for PreviewState {
    fn default() -> Self {
        PreviewState {
            wanted: None,
            since: None,
            loading_since: None,
            loaded: None,
            rx: None,
            token: None,
            rules: preview::default_preview_rules(),
            auto_highlight: true,
            archive_labels: naygo_core::archive_tree::ArchiveLabels {
                files: "archivo(s)".into(),
                folders: "carpeta(s)".into(),
                uncompressed: "sin comprimir".into(),
                more_entries: "… y más entradas".into(),
                and_more: "… y {n} más".into(),
            },
            preview_msgs: PreviewMessages::default(),
            last: None,
            mesh_scene: None,
            mesh_load_rx: None,
            mesh_render_rx: None,
            mesh_token: None,
            mesh_camera: naygo_core::mesh_preview::MeshCamera::default(),
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
        self.loading_since = None;
        if let Some(t) = self.token.take() {
            t.cancel();
        }
        if let Some(t) = self.mesh_token.take() {
            t.cancel();
        }
        self.rx = None;
        self.mesh_load_rx = None;
        self.mesh_render_rx = None;
        self.mesh_scene = None;
        self.mesh_camera = naygo_core::mesh_preview::MeshCamera::default();
        // No conservar el contenido del archivo anterior bajo la ruta recién enfocada: además de
        // ser engañoso, impedía saber que el preview actual llevaba demasiado tiempo cargando.
        self.loaded = None;
        self.last = None;
        true
    }

    /// Refleja el toggle global `Settings.auto_highlight_code`. El controlador lo sincroniza
    /// antes de lanzar el worker, así el siguiente preview respeta la preferencia actual.
    pub fn set_auto_highlight(&mut self, on: bool) {
        self.auto_highlight = on;
    }

    /// Reemplaza las reglas de clasificación con las configuradas por el usuario. El controlador
    /// las sincroniza antes de lanzar el worker; sin esto, el worker se quedaba con las reglas
    /// por defecto y las extensiones añadidas en Configuración (p. ej. `.sif`) no se
    /// previsualizaban nunca. Solo asigna si cambiaron (evita trabajo en cada tick).
    pub fn set_rules(&mut self, rules: Vec<PreviewRule>) {
        if self.rules != rules {
            self.rules = rules;
        }
    }

    /// Actualiza las etiquetas traducidas para el preview de comprimidos. El hilo de UI las
    /// resuelve con `c.config.t(...)` y llama este setter al arrancar y al cambiar el idioma.
    pub fn set_archive_labels(&mut self, labels: naygo_core::archive_tree::ArchiveLabels) {
        self.archive_labels = labels;
    }

    /// Actualiza los mensajes de error del preview. El hilo de UI los resuelve con
    /// `c.config.t(...)` y llama este setter al arrancar y al cambiar el idioma.
    pub fn set_preview_msgs(&mut self, msgs: PreviewMessages) {
        self.preview_msgs = msgs;
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
        let archive_labels = self.archive_labels.clone();
        let preview_msgs = self.preview_msgs.clone();
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let payload = build_payload(
                &path,
                &rules,
                auto_highlight,
                &archive_labels,
                &preview_msgs,
                &worker_token,
            );
            let _ = tx.send((path, payload));
        });
        self.token = Some(token);
        self.rx = Some(rx);
        self.loading_since = Some(Instant::now());
    }

    /// Drena el worker (sin bloquear). Si llegó un resultado para el path aún enfocado, lo
    /// devuelve y marca `loaded`; un resultado obsoleto se descarta. None si nada listo.
    pub fn poll(&mut self) -> Option<Payload> {
        let payload = self.poll_primary();
        if payload.is_some() {
            return payload;
        }
        self.poll_mesh()
    }

    fn poll_primary(&mut self) -> Option<Payload> {
        let rx = self.rx.as_ref()?;
        match rx.try_recv() {
            Ok((path, payload)) => {
                self.rx = None;
                self.token = None;
                self.loading_since = None;
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
                self.loading_since = None;
                None
            }
        }
    }

    /// Indica si el archivo actual es una malla que acepta interacción. La miniatura inicial de
    /// un 3MF sigue siendo válida; la geometría real se carga recién al primer gesto.
    pub fn mesh_interactive(&self) -> bool {
        self.loaded
            .as_ref()
            .or(self.wanted.as_ref())
            .and_then(|path| path.extension())
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("stl") || ext.eq_ignore_ascii_case("3mf"))
    }

    /// Ajusta la cámara desde la UI. Nunca lee disco en el hilo de UI: si la escena aún no está en
    /// memoria, arranca un worker de carga; si ya está, rasteriza en otro worker.
    pub fn orbit_mesh(&mut self, delta_x: f32, delta_y: f32) -> bool {
        if !self.mesh_interactive() {
            return false;
        }
        self.mesh_camera.yaw_degrees += delta_x * 0.45;
        self.mesh_camera.pitch_degrees =
            (self.mesh_camera.pitch_degrees - delta_y * 0.35).clamp(-85.0, 85.0);
        self.request_mesh_render()
    }

    /// Aplica zoom exponencial suave para que rueda y touchpad se sientan iguales en modelos muy
    /// grandes y muy chicos. El clamp final también existe en core como defensa adicional.
    pub fn zoom_mesh(&mut self, wheel_delta: f32) -> bool {
        if !self.mesh_interactive() {
            return false;
        }
        self.mesh_camera.zoom =
            (self.mesh_camera.zoom * (1.0 + wheel_delta * 0.0015)).clamp(0.25, 4.0);
        self.request_mesh_render()
    }

    /// Restaura la cámara ortográfica inicial del STL/3MF. La geometría cacheada se reutiliza;
    /// solo se encola un nuevo raster en el worker.
    pub fn reset_mesh_camera(&mut self) -> bool {
        if !self.mesh_interactive() {
            return false;
        }
        self.mesh_camera = naygo_core::mesh_preview::MeshCamera::default();
        self.request_mesh_render()
    }

    /// Indica que el preview enfocado está esperando contenido (sin mostrar el del archivo
    /// anterior). La UI lo usa para una señal de carga discreta y el botón Cancelar tardío.
    pub fn loading(&self) -> bool {
        self.loading_since.is_some() && self.last.is_none()
    }

    /// El botón se ofrece después de cinco segundos, no antes, y solo si aún no hay contenido.
    pub fn can_cancel(&self, now: Instant) -> bool {
        self.loading()
            && self
                .loading_since
                .is_some_and(|started| now.duration_since(started) >= PREVIEW_CANCEL_AFTER)
    }

    /// Cancela todos los workers del archivo enfocado y deja un resultado terminal para evitar
    /// que `drive_preview` lance el mismo preview otra vez en el siguiente tick.
    pub fn cancel(&mut self) -> bool {
        let active =
            self.rx.is_some() || self.mesh_load_rx.is_some() || self.mesh_render_rx.is_some();
        if let Some(token) = self.token.take() {
            token.cancel();
        }
        if let Some(token) = self.mesh_token.take() {
            token.cancel();
        }
        self.rx = None;
        self.mesh_load_rx = None;
        self.mesh_render_rx = None;
        self.loading_since = None;
        if active {
            if let Some(path) = self.wanted.clone() {
                self.loaded = Some(path);
            }
            self.last = Some(ViewCache::Message(self.preview_msgs.cancelled.clone()));
        }
        active
    }

    fn request_mesh_render(&mut self) -> bool {
        let Some(path) = self.loaded.clone().or_else(|| self.wanted.clone()) else {
            return false;
        };
        if let Some((scene_path, scene)) = self.mesh_scene.as_ref().cloned() {
            if scene_path == path {
                self.start_mesh_render(path, scene);
                return true;
            }
        }
        if self.mesh_load_rx.is_none() {
            let token = CancellationToken::new();
            let worker_token = token.clone();
            let load_path = path.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let scene =
                    naygo_core::mesh_preview::load_scene(&load_path, &worker_token).map(Arc::new);
                let _ = tx.send((load_path, scene));
            });
            self.mesh_token = Some(token);
            self.mesh_load_rx = Some(rx);
        }
        true
    }

    fn start_mesh_render(
        &mut self,
        path: PathBuf,
        scene: Arc<naygo_core::mesh_preview::MeshScene>,
    ) {
        if let Some(token) = self.mesh_token.take() {
            token.cancel();
        }
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let camera = self.mesh_camera;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let preview =
                naygo_core::mesh_preview::render_scene(&scene, 800, 600, camera, &worker_token);
            let _ = tx.send((path, preview));
        });
        self.mesh_token = Some(token);
        self.mesh_render_rx = Some(rx);
    }

    fn poll_mesh(&mut self) -> Option<Payload> {
        if let Some(rx) = self.mesh_load_rx.as_ref() {
            match rx.try_recv() {
                Ok((path, Ok(scene))) => {
                    self.mesh_load_rx = None;
                    if Some(&path) == self.loaded.as_ref().or(self.wanted.as_ref()) {
                        self.mesh_scene = Some((path.clone(), scene.clone()));
                        self.start_mesh_render(path, scene);
                    }
                }
                Ok((_path, Err(error))) => {
                    self.mesh_load_rx = None;
                    self.mesh_token = None;
                    return Some(mesh_error_payload(error, &self.preview_msgs));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.mesh_load_rx = None;
                    self.mesh_token = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        let rx = self.mesh_render_rx.as_ref()?;
        match rx.try_recv() {
            Ok((path, Ok(preview)))
                if Some(&path) == self.loaded.as_ref().or(self.wanted.as_ref()) =>
            {
                self.mesh_render_rx = None;
                self.mesh_token = None;
                let payload = Payload::Image {
                    rgba: preview.rgba,
                    width: preview.width,
                    height: preview.height,
                };
                self.last = Some(match &payload {
                    Payload::Image {
                        rgba,
                        width,
                        height,
                    } => ViewCache::Image {
                        rgba: rgba.clone(),
                        width: *width,
                        height: *height,
                    },
                    _ => unreachable!(),
                });
                Some(payload)
            }
            Ok((_path, Err(error))) => {
                self.mesh_render_rx = None;
                self.mesh_token = None;
                self.loading_since = None;
                Some(mesh_error_payload(error, &self.preview_msgs))
            }
            Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.mesh_render_rx = None;
                self.mesh_token = None;
                None
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
        }
    }

    /// ¿Hay trabajo pendiente (worker en vuelo o debounce sin vencer)? Para que el timer
    /// siga vivo hasta entregar el preview.
    pub fn busy(&self) -> bool {
        self.rx.is_some()
            || self.mesh_load_rx.is_some()
            || self.mesh_render_rx.is_some()
            || match (&self.wanted, &self.loaded) {
                (Some(w), loaded) => Some(w) != loaded.as_ref(),
                _ => false,
            }
    }
}

fn mesh_error_payload(
    error: naygo_core::mesh_preview::MeshError,
    msgs: &PreviewMessages,
) -> Payload {
    match error {
        naygo_core::mesh_preview::MeshError::Cancelled => Payload::Message(msgs.cancelled.clone()),
        naygo_core::mesh_preview::MeshError::TooLarge
        | naygo_core::mesh_preview::MeshError::TooManyTriangles => {
            Payload::Message(msgs.image_big.clone())
        }
        _ => Payload::Message(msgs.decode.clone()),
    }
}

/// Construye el payload leyendo/decodificando en el hilo del worker.
fn build_payload(
    path: &Path,
    rules: &[PreviewRule],
    auto_highlight: bool,
    archive_labels: &naygo_core::archive_tree::ArchiveLabels,
    msgs: &PreviewMessages,
    token: &CancellationToken,
) -> Payload {
    // Los comprimidos (zip/tar/tar.gz) muestran su índice como árbol ASCII, antes de la
    // clasificación normal.
    if is_archive(path) {
        return read_archive_listing(path, archive_labels, msgs);
    }
    match preview::classify_rules(path, rules) {
        PreviewKind::None => Payload::Message(msgs.not_previewable.clone()),
        // Si la regla fuerza un lenguaje y el toggle global está activo, `code_lang_for` lo
        // devuelve y el texto se resalta; con el toggle en `false` cae a texto plano.
        PreviewKind::Text => read_text(
            path,
            preview::code_lang_for(path, rules, auto_highlight),
            msgs,
        ),
        PreviewKind::Image => read_image(path, token, msgs),
        PreviewKind::Svg => read_svg(path, token, msgs),
        PreviewKind::Pdf => read_pdf(path, msgs),
        PreviewKind::Mesh => read_mesh(path, token, msgs),
    }
}

/// Rasteriza STL/3MF en CPU. El core impone límites de bytes/triángulos y consulta el token.
fn read_mesh(path: &Path, token: &CancellationToken, msgs: &PreviewMessages) -> Payload {
    match naygo_core::mesh_preview::render_file(path, 800, 600, token) {
        Ok(preview) => Payload::Image {
            rgba: preview.rgba,
            width: preview.width,
            height: preview.height,
        },
        Err(naygo_core::mesh_preview::MeshError::Cancelled) => {
            Payload::Message(msgs.cancelled.clone())
        }
        Err(naygo_core::mesh_preview::MeshError::TooLarge)
        | Err(naygo_core::mesh_preview::MeshError::TooManyTriangles) => {
            Payload::Message(msgs.image_big.clone())
        }
        Err(_) => Payload::Message(msgs.decode.clone()),
    }
}

/// Formato de archivo comprimido detectado por extensión.
enum ArchiveFormat {
    Zip,
    Tar,
    TarGz,
}

/// Detecta el formato por extensión (case-insensitive). `None` si no es comprimido legible.
fn archive_format(path: &Path) -> Option<ArchiveFormat> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Some(ArchiveFormat::TarGz)
    } else if name.ends_with(".tar") {
        Some(ArchiveFormat::Tar)
    } else if name.ends_with(".zip") {
        Some(ArchiveFormat::Zip)
    } else {
        None
    }
}

/// `true` si el archivo es un comprimido con preview (zip/tar/tar.gz/tgz).
fn is_archive(path: &Path) -> bool {
    archive_format(path).is_some()
}

/// Cuántas entradas listar como máximo (evita textos enormes en archivos con miles).
const ARCHIVE_MAX_ENTRIES: usize = 500;

/// Lee el índice de un comprimido (zip/tar/tar.gz) y devuelve el texto del preview (encabezado
/// + árbol ASCII) vía `naygo_core::archive_tree`. Tolerante: ilegible/corrupto → `Payload::Message`.
///
/// No extrae contenido (solo el índice central). `labels` son los textos ya traducidos por el
/// hilo de UI (core no conoce el catálogo i18n).
fn read_archive_listing(
    path: &Path,
    labels: &naygo_core::archive_tree::ArchiveLabels,
    msgs: &PreviewMessages,
) -> Payload {
    use naygo_core::archive_tree::{render_archive_tree, ArchiveSummary};
    let Some(fmt) = archive_format(path) else {
        return Payload::Message(msgs.not_previewable.clone());
    };
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let mut entries = Vec::new();
    let mut summary = ArchiveSummary::default();
    let read_ok = match fmt {
        ArchiveFormat::Zip => read_zip_entries(path, &mut entries, &mut summary),
        ArchiveFormat::Tar => read_tar_entries(path, &mut entries, &mut summary, false),
        ArchiveFormat::TarGz => read_tar_entries(path, &mut entries, &mut summary, true),
    };
    if !read_ok {
        return Payload::Message(msgs.archive_bad.clone());
    }
    let text = render_archive_tree(
        &entries,
        &summary,
        &name,
        naygo_core::format::SizeFormat::Auto,
        labels,
    );
    Payload::Text {
        text,
        truncated: summary.truncated,
        highlighted: None,
    }
}

/// Lee las entradas de un .zip. false si no se pudo abrir/leer.
fn read_zip_entries(
    path: &Path,
    entries: &mut Vec<naygo_core::archive_tree::ArchiveEntry>,
    summary: &mut naygo_core::archive_tree::ArchiveSummary,
) -> bool {
    use naygo_core::archive_tree::ArchiveEntry;
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    let total = archive.len();
    summary.total_entries = total;
    let shown = total.min(ARCHIVE_MAX_ENTRIES);
    summary.truncated = total > shown;
    for i in 0..shown {
        let Ok(entry) = archive.by_index(i) else {
            continue;
        };
        let is_dir = entry.is_dir();
        let size = entry.size();
        if is_dir {
            summary.dirs += 1;
        } else {
            summary.files += 1;
            summary.total_uncompressed += size;
        }
        entries.push(ArchiveEntry {
            path: entry.name().to_string(),
            is_dir,
            size,
        });
    }
    true
}

/// Lee las entradas de un .tar o .tar.gz (si `gz`, descomprime con flate2). false si falla.
/// tar es secuencial: no se sabe el total sin iterar todo, así que al cortar en el tope solo
/// marcamos `truncated` (sin un total exacto). `render_archive_tree` lo refleja como "… y más
/// entradas" cuando no hay un número confiable, en vez de mentir un conteo.
fn read_tar_entries(
    path: &Path,
    entries: &mut Vec<naygo_core::archive_tree::ArchiveEntry>,
    summary: &mut naygo_core::archive_tree::ArchiveSummary,
    gz: bool,
) -> bool {
    use naygo_core::archive_tree::ArchiveEntry;
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let reader: Box<dyn std::io::Read> = if gz {
        Box::new(flate2::read::GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut archive = tar::Archive::new(reader);
    let Ok(iter) = archive.entries() else {
        return false;
    };
    let mut had_error = false;
    for entry in iter {
        if entries.len() >= ARCHIVE_MAX_ENTRIES {
            summary.truncated = true;
            break;
        }
        let e = match entry {
            Ok(e) => e,
            Err(_) => {
                had_error = true;
                continue;
            }
        };
        let header = e.header();
        let is_dir = header.entry_type().is_dir();
        let size = header.size().unwrap_or(0);
        let path_str = e
            .path()
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if path_str.is_empty() {
            continue;
        }
        if is_dir {
            summary.dirs += 1;
        } else {
            summary.files += 1;
            summary.total_uncompressed += size;
        }
        entries.push(ArchiveEntry {
            path: path_str,
            is_dir,
            size,
        });
    }
    // Si no se pudo leer ninguna entrada Y hubo errores, el archivo es corrupto/inválido.
    if entries.is_empty() && had_error {
        return false;
    }
    // Si hubo errores pero alcanzamos a leer algunas entradas, el listado es parcial:
    // marcar truncado para que el preview muestre "… y más entradas" (honesto).
    if had_error && !entries.is_empty() {
        summary.truncated = true;
    }
    // No sumamos un "+1" engañoso: dejamos `total_entries == entries.len()` y confiamos en
    // `truncated` para que el preview muestre "… y más entradas" (sin un número falso).
    summary.total_entries = entries.len();
    true
}

/// Lee el archivo como texto y, si `lang` está presente (la regla fuerza un lenguaje), resalta
/// el texto YA recortado con `core::highlight` (defensa i16 intacta: se resalta lo recortado).
fn read_text(path: &Path, lang: Option<CodeLang>, msgs: &PreviewMessages) -> Payload {
    use naygo_core::preview::{truncate_text, TEXT_MAX_BYTES};
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return Payload::Message(msgs.read.clone());
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
            Err(_) => return Payload::Message(msgs.read.clone()),
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

fn read_image(path: &Path, token: &CancellationToken, msgs: &PreviewMessages) -> Payload {
    use naygo_core::preview::{IMAGE_MAX_BYTES, IMAGE_MAX_SIDE};
    match std::fs::metadata(path) {
        Ok(m) if m.len() > IMAGE_MAX_BYTES => return Payload::Message(msgs.image_big.clone()),
        Ok(_) => {}
        Err(_) => return Payload::Message(msgs.read.clone()),
    }
    if token.is_cancelled() {
        return Payload::Message(msgs.cancelled.clone());
    }
    let Ok(img) = image::open(path) else {
        return Payload::Message(msgs.decode.clone());
    };
    if token.is_cancelled() {
        return Payload::Message(msgs.cancelled.clone());
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
fn read_svg(path: &Path, token: &CancellationToken, msgs: &PreviewMessages) -> Payload {
    use naygo_core::preview::{IMAGE_MAX_BYTES, IMAGE_MAX_SIDE};
    let bytes = match std::fs::metadata(path) {
        Ok(m) if m.len() > IMAGE_MAX_BYTES => return Payload::Message(msgs.svg_big.clone()),
        Ok(_) => match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return Payload::Message(msgs.read.clone()),
        },
        Err(_) => return Payload::Message(msgs.read.clone()),
    };
    if token.is_cancelled() {
        return Payload::Message(msgs.cancelled.clone());
    }
    // usvg parsea el SVG a un árbol simplificado. Opciones por defecto (sin fuentes externas:
    // el texto del SVG usa la BD de fuentes por defecto, suficiente para un preview).
    let opt = usvg::Options::default();
    let tree = match usvg::Tree::from_data(&bytes, &opt) {
        Ok(t) => t,
        Err(_) => return Payload::Message(msgs.svg_bad.clone()),
    };
    if token.is_cancelled() {
        return Payload::Message(msgs.cancelled.clone());
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
        None => return Payload::Message(msgs.rasterize.clone()),
    };
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    if token.is_cancelled() {
        return Payload::Message(msgs.cancelled.clone());
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
fn read_pdf(path: &Path, msgs: &PreviewMessages) -> Payload {
    match std::fs::metadata(path) {
        Ok(m) if m.len() > PDF_MAX_BYTES => return Payload::Message(msgs.pdf_big.clone()),
        Ok(_) => {}
        Err(_) => return Payload::Message(msgs.read.clone()),
    }
    // Nº de páginas vía lopdf (barato: solo recorre el árbol de páginas).
    let pages = lopdf::Document::load(path)
        .ok()
        .map(|d| d.get_pages().len());
    // Frase traducida con el nº de páginas (placeholder `{n}`). El código añade la puntuación
    // final (`.` sin texto, `:` con texto) y los saltos; así una sola clave i18n cubre ambas.
    let pages_phrase = |n: usize| msgs.pdf_pages.replace("{n}", &n.to_string());
    // `pdf-extract` 0.10 puede hacer panic ante streams sintácticamente válidos pero con
    // operandos faltantes. El PDF es input hostil: lo convertimos en el mismo resultado discreto
    // que un error normal y silenciamos el panic hook solo alrededor de esta llamada recuperable.
    let extracted = crate::logging::catch_recoverable_panic(|| pdf_extract::extract_text(path));
    let text = match extracted {
        Ok(Ok(t)) => t,
        Ok(Err(error)) => {
            crate::logging::log_line(&format!(
                "preview PDF: no se pudo extraer texto de {}: {error}",
                path.display()
            ));
            let head = match pages {
                Some(n) => format!("{}.\n\n", pages_phrase(n)),
                None => String::new(),
            };
            return Payload::Text {
                text: format!("{head}{}", msgs.pdf_no_text),
                truncated: false,
                highlighted: None,
            };
        }
        Err(_) => {
            crate::logging::log_line(&format!(
                "preview PDF: pdf-extract hizo panic controlado para {}",
                path.display()
            ));
            let head = match pages {
                Some(n) => format!("{}.\n\n", pages_phrase(n)),
                None => String::new(),
            };
            return Payload::Text {
                text: format!("{head}{}", msgs.pdf_no_text),
                truncated: false,
                highlighted: None,
            };
        }
    };
    let header = match pages {
        Some(n) => format!("{}:\n\n", pages_phrase(n)),
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

    /// Etiquetas en español para los tests de archive listing.
    fn labels_es() -> naygo_core::archive_tree::ArchiveLabels {
        naygo_core::archive_tree::ArchiveLabels {
            files: "archivo(s)".into(),
            folders: "carpeta(s)".into(),
            uncompressed: "sin comprimir".into(),
            more_entries: "… y más entradas".into(),
            and_more: "… y {n} más".into(),
        }
    }

    /// Mensajes de error en español para los tests (valores por defecto del struct).
    fn msgs_es() -> PreviewMessages {
        PreviewMessages::default()
    }

    #[test]
    fn pdf_con_operador_sin_operandos_no_panica() {
        use lopdf::content::{Content, Operation};
        use lopdf::{Dictionary, Document, Object, Stream};

        // Regresión del panic real de pdf-extract: el operador `w` indexa operands[0] sin
        // comprobarlo. El PDF sigue siendo parseable por lopdf, pero su stream es hostil.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("operando-faltante.pdf");
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content = Content {
            operations: vec![Operation::new("w", Vec::new())],
        }
        .encode()
        .unwrap();
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content));

        let mut page = Dictionary::new();
        page.set("Type", Object::Name(b"Page".to_vec()));
        page.set("Parent", Object::Reference(pages_id));
        page.set(
            "MediaBox",
            Object::Array(vec![0.into(), 0.into(), 612.into(), 792.into()]),
        );
        page.set("Resources", Object::Dictionary(Dictionary::new()));
        page.set("Contents", Object::Reference(content_id));
        let page_id = doc.add_object(page);

        let mut pages = Dictionary::new();
        pages.set("Type", Object::Name(b"Pages".to_vec()));
        pages.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        pages.set("Count", Object::Integer(1));
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let mut catalog = Dictionary::new();
        catalog.set("Type", Object::Name(b"Catalog".to_vec()));
        catalog.set("Pages", Object::Reference(pages_id));
        let catalog_id = doc.add_object(catalog);
        doc.trailer.set("Root", Object::Reference(catalog_id));
        doc.save(&path).unwrap();

        match read_pdf(&path, &msgs_es()) {
            Payload::Text { text, .. } => assert!(text.contains("No se pudo extraer")),
            other => panic!("esperaba degradación a texto informativo, fue {other:?}"),
        }
    }

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
    fn cancelar_preview_se_ofrece_solo_despues_de_cinco_segundos_sin_contenido() {
        let mut s = PreviewState::new();
        let t0 = Instant::now();
        s.set_wanted(Some(PathBuf::from("C:/x/lento.stl")), t0);
        // Simula el instante en que ya arrancó un worker (el test no toca disco ni hilos).
        s.loading_since = Some(t0);
        assert!(s.loading());
        assert!(!s.can_cancel(t0 + PREVIEW_CANCEL_AFTER - Duration::from_millis(1)));
        assert!(s.can_cancel(t0 + PREVIEW_CANCEL_AFTER));
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
        match read_archive_listing(&p, &labels_es(), &msgs_es()) {
            Payload::Text { text, .. } => {
                assert!(text.contains("readme.txt"), "lista readme: {text}");
                // En el árbol ASCII la carpeta "src" aparece como "src/" y el archivo como "main.rs"
                assert!(text.contains("src"), "lista src: {text}");
                assert!(text.contains("main.rs"), "lista main.rs: {text}");
                assert!(text.contains("2 archivo"), "cuenta archivos: {text}");
            }
            other => panic!("esperaba texto, fue {other:?}"),
        }
    }

    #[test]
    fn zip_invalido_da_mensaje() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("roto.zip");
        std::fs::write(&p, b"esto no es un zip").unwrap();
        match read_archive_listing(&p, &labels_es(), &msgs_es()) {
            Payload::Message(_) => {}
            other => panic!("esperaba mensaje, fue {other:?}"),
        }
    }

    #[test]
    fn read_archive_listing_zip_muestra_entradas() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("demo.zip");
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(file);
            let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
            zw.start_file("carpeta/hola.txt", opts).unwrap();
            zw.write_all(b"hola mundo").unwrap();
            zw.finish().unwrap();
        }
        let payload = read_archive_listing(&zip_path, &labels_es(), &msgs_es());
        match payload {
            Payload::Text { text, .. } => {
                assert!(text.contains("hola.txt"));
                assert!(text.contains("carpeta"));
                assert!(text.contains("archivo"));
            }
            _ => panic!("se esperaba Payload::Text"),
        }
    }

    #[test]
    fn read_archive_listing_targz_muestra_entradas() {
        let dir = tempfile::tempdir().unwrap();
        let tgz_path = dir.path().join("demo.tar.gz");
        {
            let file = std::fs::File::create(&tgz_path).unwrap();
            let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            let mut tar = tar::Builder::new(enc);
            let data = b"contenido";
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_cksum();
            tar.append_data(&mut header, "dir/archivo.txt", &data[..])
                .unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }
        let payload = read_archive_listing(&tgz_path, &labels_es(), &msgs_es());
        match payload {
            Payload::Text { text, .. } => {
                assert!(text.contains("archivo.txt"), "lista el archivo del tar.gz");
                assert!(text.contains("dir"), "lista la carpeta del tar.gz");
            }
            _ => panic!("se esperaba Payload::Text, fue {:?}", payload),
        }
    }

    #[test]
    fn read_archive_listing_targz_corrupto_es_message() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("malo.tar.gz");
        // bytes que no son un gzip válido → GzDecoder/tar fallan → Message, no panic
        std::fs::write(&bad, b"esto no es un gzip valido").unwrap();
        let payload = read_archive_listing(&bad, &labels_es(), &msgs_es());
        assert!(
            matches!(payload, Payload::Message(_)),
            "tar.gz dañado => Message, no panic"
        );
    }

    #[test]
    fn texto_se_lee_y_trunca() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("hola.txt");
        std::fs::write(&p, b"linea1\nlinea2\n").unwrap();
        let rules = preview::default_preview_rules();
        let token = CancellationToken::new();
        match build_payload(&p, &rules, true, &labels_es(), &msgs_es(), &token) {
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
        match build_payload(&p, &rules, true, &labels_es(), &msgs_es(), &token) {
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

        match build_payload(&p, &rules, true, &labels_es(), &msgs_es(), &token) {
            Payload::Text { highlighted, .. } => {
                assert!(
                    highlighted.is_some(),
                    "con el toggle activo el código se resalta"
                );
            }
            other => panic!("esperaba texto, fue {other:?}"),
        }

        match build_payload(&p, &rules, false, &labels_es(), &msgs_es(), &token) {
            Payload::Text { highlighted, .. } => {
                assert!(
                    highlighted.is_none(),
                    "con el toggle apagado no debe resaltar"
                );
            }
            other => panic!("esperaba texto, fue {other:?}"),
        }
    }

    #[test]
    fn set_rules_reemplaza_las_reglas_del_worker() {
        // Regresión: el worker arrancaba con default_preview_rules() y nunca recibía las reglas
        // del usuario, así que una extensión añadida (.sif) no se previsualizaba. set_rules debe
        // reemplazarlas para que el siguiente preview las respete.
        let mut st = PreviewState::new();
        let nuevas = vec![preview::PreviewRule {
            ext: "sif".to_string(),
            enabled: true,
            view: preview::ViewMode::Text,
        }];
        st.set_rules(nuevas.clone());
        assert_eq!(
            st.rules, nuevas,
            "set_rules reemplaza las reglas del worker"
        );
    }

    #[test]
    fn sif_tratado_como_texto_se_previsualiza() {
        // Con una regla .sif→Text (lo que configura el usuario), el contenido se lee como texto.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("modelo.sif");
        std::fs::write(&p, b"linea de texto\n").unwrap();
        let rules = vec![preview::PreviewRule {
            ext: "sif".to_string(),
            enabled: true,
            view: preview::ViewMode::Text,
        }];
        let token = CancellationToken::new();
        match build_payload(&p, &rules, false, &labels_es(), &msgs_es(), &token) {
            Payload::Text { text, .. } => assert!(text.contains("linea de texto")),
            other => panic!("esperaba texto para .sif, fue {other:?}"),
        }
    }
}
