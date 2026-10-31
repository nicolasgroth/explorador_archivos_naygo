// Naygo — preview liviano para la UI Slint: worker con debounce + cancelación.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Al enfocar un archivo se espera un debounce (150 ms)
// y recién entonces un worker lee/decodifica en su hilo. El worker produce un `Payload`
// con bytes crudos (texto ya truncado, o RGBA ya escalada): NUNCA construye tipos de Slint
// (eso ocurre en el hilo de UI, que arma el `slint::Image`). Cada worker es cancelable: si
// el foco cambia, el token se cancela y el receptor exclusivo de esa solicitud se descarta.
// A → B → A nunca comparte canal. Las plazas se liberan al terminar el hilo, no al cancelar.

#[path = "preview_animation.rs"]
mod animation;
#[path = "preview_animation_reader.rs"]
mod animation_reader;
#[path = "preview_isolated.rs"]
mod isolated;
#[cfg(test)]
#[path = "preview_lifecycle_tests.rs"]
mod lifecycle_tests;
#[path = "preview_workers.rs"]
mod workers;
pub use isolated::dispatch as isolated_dispatch;

use naygo_core::cancel::CancellationToken;
use naygo_core::highlight::HlLine;
use naygo_core::preview::{self, CodeLang, PreviewKind, PreviewRule};
use naygo_core::preview_policy::{Quality, Reservation, RetainedBudget};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc::Receiver, Arc};
use std::time::{Duration, Instant};

/// Debounce: mover rápido por una carpeta NO dispara una lectura por archivo.
pub const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(150);

/// Última vista entregada, cacheada para pintar en cada tick sin reconstruir el worker
/// (evita parpadeo entre el momento en que llega el resultado y el siguiente foco). Guarda
/// texto acotado y un handle de imagen construido UNA vez en UI; los VM lo comparten.
#[derive(Debug)]
pub enum ViewCache {
    Text {
        text: String,
        truncated: bool,
        bytes_read: Option<usize>,
        /// Líneas ya resaltadas si la regla forzó un lenguaje de código; `None` = texto plano.
        highlighted: Option<Vec<HlLine>>,
        _reservation: Reservation,
    },
    Image {
        image: slint::Image,
        _reservation: Reservation,
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
        bytes_read: Option<usize>,
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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PreviewMessages {
    pub reader_stopped: String,
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
            reader_stopped: "El lector se detuvo por un error o límite de recursos. Puedes reintentar o abrir el archivo externamente.".into(),
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
    Result<Arc<RetainedScene>, naygo_core::mesh_preview::MeshError>,
);
/// El permiso vive tanto como el último renderer que todavía usa la geometría.
struct RetainedScene {
    scene: naygo_core::mesh_preview::MeshScene,
    _reservation: Reservation,
}
type MeshRenderMessage = (
    PathBuf,
    Result<naygo_core::mesh_preview::MeshPreview, naygo_core::mesh_preview::MeshError>,
);

/// Estado del preview: qué se quiere, desde cuándo (debounce), qué está cargado, el worker
/// en vuelo y su token. Es propiedad del controlador.
pub struct PreviewState {
    quality: Quality,
    /// Compartido por imágenes/escenas de todos los paneles de preview. No representa RSS.
    retained: RetainedBudget,
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
    /// Límite de lectura textual elegido por el usuario, acotado a las opciones configurables.
    text_max_bytes: usize,
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
    mesh_scene: Option<(PathBuf, Arc<RetainedScene>)>,
    /// Worker que carga la geometría real al iniciar la primera interacción 3D.
    mesh_load_rx: Option<Receiver<MeshLoadMessage>>,
    /// Worker de rasterización de la escena ya cacheada.
    mesh_render_rx: Option<Receiver<MeshRenderMessage>>,
    /// Token compartido por la carga/rasterización 3D; se cancela al cambiar de archivo.
    mesh_token: Option<CancellationToken>,
    /// Cámara acumulada por arrastre/rueda. El último gesto reemplaza el render pendiente.
    mesh_camera: naygo_core::mesh_preview::MeshCamera,
    mesh_appearance: naygo_core::mesh_preview::MeshAppearance,
    workers: workers::Workers,
    /// Sólo una cámara pendiente; los gestos sustituyen el último pedido, no crean hilos.
    mesh_dirty: bool,
    /// Cancelar/error son terminales hasta Reintentar o elegir otro archivo.
    terminal: bool,
    visible: bool,
    animation: Option<animation::Playback>,
}

impl Default for PreviewState {
    fn default() -> Self {
        PreviewState {
            quality: Quality::default(),
            retained: RetainedBudget::new(64 * 1024 * 1024),
            wanted: None,
            since: None,
            loaded: None,
            rx: None,
            token: None,
            rules: preview::default_preview_rules(),
            text_max_bytes: preview::DEFAULT_TEXT_MAX_BYTES as usize,
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
            mesh_appearance: naygo_core::mesh_preview::MeshAppearance::default(),
            workers: workers::Workers::default(),
            mesh_dirty: false,
            terminal: false,
            visible: true,
            animation: None,
        }
    }
}

impl PreviewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cambiar calidad invalida sólo la generación del preview. Conservar el budget evita
    /// liberar ficticiamente reservas de escenas que aún usan workers cancelados.
    pub fn set_economical(&mut self, economical: bool, now: Instant) {
        if self.quality.economical == economical {
            return;
        }
        self.quality.economical = economical;
        let wanted = self.wanted.clone();
        self.set_wanted(None, now);
        self.set_wanted(wanted, now);
    }

    /// Construye UN buffer Slint por resultado. Los VM/paneles clonan el handle compartido,
    /// no RGBA por tick. Reserva 2× RGBA para admitir el buffer y su copia de presentación.
    fn cache_image(&self, rgba: &[u8], width: u32, height: u32) -> Option<ViewCache> {
        let bytes = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        if width == 0
            || height == 0
            || width > preview::IMAGE_MAX_SIDE
            || height > preview::IMAGE_MAX_SIDE
            || bytes != rgba.len()
        {
            return None;
        }
        let reservation = self.retained.reserve(bytes.checked_mul(2)?)?;
        let buffer = slint::SharedPixelBuffer::clone_from_slice(rgba, width, height);
        Some(ViewCache::Image {
            image: slint::Image::from_rgba8(buffer),
            _reservation: reservation,
        })
    }

    fn cache_text(
        &self,
        text: &str,
        truncated: bool,
        bytes_read: Option<usize>,
        highlighted: &Option<Vec<HlLine>>,
    ) -> Option<ViewCache> {
        let mut bytes = text.len();
        if let Some(lines) = highlighted {
            bytes = bytes.checked_add(lines.len().checked_mul(std::mem::size_of::<HlLine>())?)?;
            for line in lines {
                bytes = bytes.checked_add(
                    line.spans
                        .len()
                        .checked_mul(std::mem::size_of::<naygo_core::highlight::HlSpan>())?,
                )?;
                for span in &line.spans {
                    bytes = bytes.checked_add(span.text.len())?;
                }
            }
        }
        // Reserva conservadora para copia cruda y modelos de texto. El costo del parser
        // y de gramáticas compartidas no forma parte de este presupuesto de retención.
        let reservation = self.retained.reserve(bytes.checked_mul(4)?)?;
        Some(ViewCache::Text {
            text: text.into(),
            truncated,
            bytes_read,
            highlighted: highlighted.clone(),
            _reservation: reservation,
        })
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
        if let Some(t) = self.mesh_token.take() {
            t.cancel();
        }
        self.rx = None;
        self.animation = None;
        self.mesh_load_rx = None;
        self.mesh_render_rx = None;
        self.mesh_scene = None;
        self.mesh_camera = naygo_core::mesh_preview::MeshCamera::default();
        self.mesh_appearance = naygo_core::mesh_preview::MeshAppearance::default();
        self.mesh_dirty = false;
        self.terminal = false;
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
            let changed_kind = self.wanted.as_ref().is_some_and(|path| {
                preview::classify_rules(path, &self.rules) != preview::classify_rules(path, &rules)
            });
            self.rules = rules;
            if changed_kind {
                // Disabling/changing the active reader is explicit user input. Retire its
                // generation immediately; unrelated rule edits do not restart playback.
                let wanted = self.wanted.clone();
                let now = Instant::now();
                self.set_wanted(None, now);
                self.set_wanted(wanted, now);
            }
        }
    }

    /// Cambia el límite de lectura textual y reinicia solo el preview de texto seleccionado.
    /// El worker aislado sigue sujeto al límite de proceso y a la admisión global de memoria.
    pub fn set_text_max_bytes(&mut self, bytes: u32) {
        let bytes = preview::normalize_text_max_bytes(bytes) as usize;
        if self.text_max_bytes == bytes {
            return;
        }
        self.text_max_bytes = bytes;
        let is_text = self
            .wanted
            .as_ref()
            .is_some_and(|path| preview::classify_rules(path, &self.rules) == PreviewKind::Text);
        if is_text {
            let wanted = self.wanted.clone();
            let now = Instant::now();
            self.set_wanted(None, now);
            self.set_wanted(wanted, now);
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
        if !self.visible || self.terminal || self.animation.is_some() {
            return false;
        }
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
        if !self.visible
            || self.terminal
            || self.rx.is_some()
            || self.animation.is_some()
            || !self.workers.has_capacity()
        {
            return;
        }
        let Some(path) = self.wanted.clone() else {
            return;
        };
        if !is_archive(&path) && preview::classify_rules(&path, &self.rules) == PreviewKind::Image {
            match animation::Playback::start(
                &self.workers,
                path,
                self.preview_msgs.clone(),
                self.quality,
            ) {
                Ok(playback) => self.animation = playback,
                Err(_) => {
                    self.fail();
                }
            }
            return;
        }
        let rules = self.rules.clone();
        let auto_highlight = self.auto_highlight;
        let quality = self.quality;
        let text_max_bytes = self.text_max_bytes;
        let archive_labels = self.archive_labels.clone();
        let preview_msgs = self.preview_msgs.clone();
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let started = Instant::now();
        let result = self.workers.spawn(move || {
            let payload = isolated::build(
                &path,
                &rules,
                auto_highlight,
                isolated::BuildOptions {
                    quality,
                    text_max_bytes,
                },
                &archive_labels,
                &preview_msgs,
                &worker_token,
            );
            crate::metrics::record("preview.build", started.elapsed());
            (path, payload)
        });
        match result {
            Ok(Some(rx)) => {
                self.token = Some(token);
                self.rx = Some(rx);
            }
            Ok(None) => {} // capacidad retenida por workers anteriores: conservar sólo wanted.
            Err(_) => {
                self.fail();
            }
        }
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
                if Some(&path) == self.wanted.as_ref() {
                    self.terminal = matches!(&payload, Payload::Message(_));
                    self.loaded = Some(path);
                    self.last = Some(match &payload {
                        Payload::Text {
                            text,
                            truncated,
                            bytes_read,
                            highlighted,
                        } => match self.cache_text(text, *truncated, *bytes_read, highlighted) {
                            Some(cache) => cache,
                            None => return Some(self.fail_resources()),
                        },
                        Payload::Image {
                            rgba,
                            width,
                            height,
                        } => match self.cache_image(rgba, *width, *height) {
                            Some(cache) => cache,
                            None => return Some(self.fail_resources()),
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
                Some(self.fail())
            }
        }
    }

    /// Indica si el archivo actual es una malla que acepta interacción. La miniatura inicial de
    /// un 3MF sigue siendo válida; la geometría real se carga recién al primer gesto.
    pub fn mesh_interactive(&self) -> bool {
        if self.terminal || !self.visible {
            return false;
        }
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
        // La cámara compone la órbita en sus ejes de pantalla. La orientación completa permite
        // que los siguientes gestos sigan sintiéndose directos aunque el modelo esté inclinado.
        self.mesh_camera.orbit(delta_x * 0.45, delta_y * 0.35);
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

    pub fn mesh_appearance_indices(&self) -> (i32, i32) {
        use naygo_core::mesh_preview::{MeshColor, MeshSurface};
        let color = match self.mesh_appearance.color {
            MeshColor::Blue => 0,
            MeshColor::White => 1,
            MeshColor::Warm => 2,
            MeshColor::Source => 3,
        };
        let surface = match self.mesh_appearance.surface {
            MeshSurface::Solid => 0,
            MeshSurface::Edges => 1,
            MeshSurface::Wireframe => 2,
        };
        (color, surface)
    }

    pub fn cycle_mesh_color(&mut self) -> bool {
        use naygo_core::mesh_preview::MeshColor;
        if !self.mesh_interactive() {
            return false;
        }
        self.mesh_appearance.color = match self.mesh_appearance.color {
            MeshColor::Blue => MeshColor::White,
            MeshColor::White => MeshColor::Warm,
            MeshColor::Warm => MeshColor::Source,
            MeshColor::Source => MeshColor::Blue,
        };
        self.request_mesh_render()
    }

    pub fn cycle_mesh_surface(&mut self) -> bool {
        use naygo_core::mesh_preview::MeshSurface;
        if !self.mesh_interactive() {
            return false;
        }
        self.mesh_appearance.surface = match self.mesh_appearance.surface {
            MeshSurface::Solid => MeshSurface::Edges,
            MeshSurface::Edges => MeshSurface::Wireframe,
            MeshSurface::Wireframe => MeshSurface::Solid,
        };
        self.request_mesh_render()
    }

    /// Indica que el preview enfocado está esperando contenido (sin mostrar el del archivo
    /// anterior). La UI lo usa para una señal de carga discreta; Cancelar también cubre 3D.
    pub fn loading(&self) -> bool {
        self.busy() && self.last.is_none()
    }

    /// Incluye debounce, espera de plaza y geometría/render detrás de una miniatura.
    pub fn can_cancel(&self, _now: Instant) -> bool {
        self.busy() || self.animation.as_ref().is_some_and(|a| !a.ended)
    }

    /// Cancela todos los workers del archivo enfocado y deja un resultado terminal para evitar
    /// que `drive_preview` lance el mismo preview otra vez en el siguiente tick.
    pub fn cancel(&mut self) -> bool {
        let active = self.can_cancel(Instant::now());
        self.animation = None;
        if let Some(token) = self.token.take() {
            token.cancel();
        }
        if let Some(token) = self.mesh_token.take() {
            token.cancel();
        }
        self.rx = None;
        self.mesh_load_rx = None;
        self.mesh_render_rx = None;
        self.mesh_dirty = false;
        self.mesh_scene = None;
        if active {
            self.terminal = true;
            if let Some(path) = self.wanted.clone() {
                self.loaded = Some(path);
            }
            self.last = Some(ViewCache::Message(self.preview_msgs.cancelled.clone()));
        }
        active
    }

    fn request_mesh_render(&mut self) -> bool {
        self.mesh_dirty = true;
        self.drive_mesh();
        true
    }

    fn drive_mesh(&mut self) {
        if !self.visible
            || self.terminal
            || !self.mesh_dirty
            || self.mesh_load_rx.is_some()
            || self.mesh_render_rx.is_some()
        {
            return;
        }
        let Some(path) = self.loaded.clone().or_else(|| self.wanted.clone()) else {
            return;
        };
        if let Some((scene_path, scene)) = self.mesh_scene.as_ref().cloned() {
            if scene_path == path {
                self.start_mesh_render(path, scene);
                return;
            }
        }
        if self.mesh_load_rx.is_none() {
            let token = CancellationToken::new();
            let worker_token = token.clone();
            let load_path = path.clone();
            let budget = self.retained.clone();
            let started = Instant::now();
            let result = self.workers.spawn(move || {
                let scene = isolated::load_scene(&load_path, &worker_token).and_then(|scene| {
                    use naygo_core::mesh_preview::MeshError;
                    let bytes = scene.retained_bytes().ok_or(MeshError::TooLarge)?;
                    // Leave room for a displayed frame; reject before retaining the scene.
                    if bytes > naygo_core::mesh_preview::SCENE_MAX_BYTES {
                        return Err(MeshError::TooLarge);
                    }
                    let reservation = budget.reserve(bytes).ok_or(MeshError::TooLarge)?;
                    Ok(Arc::new(RetainedScene {
                        scene,
                        _reservation: reservation,
                    }))
                });
                crate::metrics::record("preview.mesh_load", started.elapsed());
                (load_path, scene)
            });
            match result {
                Ok(Some(rx)) => {
                    self.mesh_token = Some(token);
                    self.mesh_load_rx = Some(rx);
                }
                Ok(None) => {}
                Err(_) => {
                    self.fail();
                }
            }
        }
    }

    fn start_mesh_render(&mut self, path: PathBuf, scene: Arc<RetainedScene>) {
        if let Some(token) = self.mesh_token.take() {
            token.cancel();
        }
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let camera = self.mesh_camera;
        let appearance = self.mesh_appearance;
        let (width, height) = self.quality.mesh_size();
        let started = Instant::now();
        let result = self.workers.spawn(move || {
            let preview = naygo_core::mesh_preview::render_scene_with_appearance(
                &scene.scene,
                width,
                height,
                camera,
                appearance,
                &worker_token,
            );
            crate::metrics::record("preview.mesh_render", started.elapsed());
            (path, preview)
        });
        match result {
            Ok(Some(rx)) => {
                self.mesh_dirty = false;
                self.mesh_token = Some(token);
                self.mesh_render_rx = Some(rx);
            }
            Ok(None) => {}
            Err(_) => {
                self.fail();
            }
        }
    }

    fn poll_mesh(&mut self) -> Option<Payload> {
        self.drive_mesh();
        if let Some(rx) = self.mesh_load_rx.as_ref() {
            match rx.try_recv() {
                Ok((path, Ok(scene))) => {
                    self.mesh_load_rx = None;
                    if Some(&path) == self.loaded.as_ref().or(self.wanted.as_ref()) {
                        self.mesh_scene = Some((path.clone(), scene.clone()));
                        self.mesh_dirty = true;
                        self.start_mesh_render(path, scene);
                    }
                }
                Ok((_path, Err(error))) => {
                    self.mesh_load_rx = None;
                    self.mesh_token = None;
                    return Some(self.fail_mesh(error));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.mesh_load_rx = None;
                    self.mesh_token = None;
                    return Some(self.fail());
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
                if self.mesh_dirty {
                    self.drive_mesh();
                    // Presentar este frame válido de la MISMA escena mientras se prepara la
                    // última cámara. Descartarlo en cada gesto congelaría un arrastre continuo.
                }
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
                    } => match self.cache_image(rgba, *width, *height) {
                        Some(cache) => cache,
                        None => return Some(self.fail_resources()),
                    },
                    _ => unreachable!(),
                });
                Some(payload)
            }
            Ok((_path, Err(error))) => {
                self.mesh_render_rx = None;
                self.mesh_token = None;
                Some(self.fail_mesh(error))
            }
            Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.mesh_render_rx = None;
                self.mesh_token = None;
                Some(self.fail())
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
        }
    }

    /// ¿Hay trabajo pendiente (worker en vuelo o debounce sin vencer)? Para que el timer
    /// siga vivo hasta entregar el preview.
    pub fn busy(&self) -> bool {
        if !self.visible || self.terminal {
            return false;
        }
        self.rx.is_some()
            || self.mesh_dirty
            || self.mesh_load_rx.is_some()
            || self.mesh_render_rx.is_some()
            || match (&self.wanted, &self.loaded) {
                (Some(w), loaded) => Some(w) != loaded.as_ref(),
                _ => false,
            }
    }

    fn fail_mesh(&mut self, error: naygo_core::mesh_preview::MeshError) -> Payload {
        let payload = mesh_error_payload(error, &self.preview_msgs);
        self.cancel();
        self.terminal = true;
        if let Payload::Message(message) = &payload {
            self.last = Some(ViewCache::Message(message.clone()));
        }
        payload
    }

    fn fail(&mut self) -> Payload {
        self.cancel();
        self.terminal = true;
        self.loaded = self.wanted.clone();
        let message = self.preview_msgs.decode.clone();
        self.last = Some(ViewCache::Message(message.clone()));
        Payload::Message(message)
    }

    fn fail_resources(&mut self) -> Payload {
        self.cancel();
        self.terminal = true;
        self.loaded = self.wanted.clone();
        let message = self.preview_msgs.reader_stopped.clone();
        self.last = Some(ViewCache::Message(message.clone()));
        Payload::Message(message)
    }

    pub fn can_retry(&self) -> bool {
        self.visible && self.terminal && self.wanted.is_some()
    }

    pub fn retry(&mut self, now: Instant) -> bool {
        if !self.can_retry() {
            return false;
        }
        self.terminal = false;
        self.animation = None;
        self.loaded = None;
        self.last = None;
        self.since = Some(now);
        true
    }

    /// Animation frames use a separate lightweight timer; they never pump directory rows.
    pub fn animation_tick_needed(&self) -> bool {
        self.visible && self.animation.as_ref().is_some_and(|a| a.tick_needed())
    }
    pub fn animation_state(&self) -> i32 {
        match &self.animation {
            Some(a) if a.animated && !self.terminal => {
                if a.ended {
                    3
                } else if a.paused() {
                    2
                } else {
                    1
                }
            }
            _ => 0,
        }
    }
    pub fn toggle_animation(&mut self, now: Instant) -> bool {
        match self.animation_state() {
            1 | 2 => {
                self.animation.as_mut().unwrap().toggle(now);
                true
            }
            3 => {
                self.animation = None;
                self.loaded = None;
                self.last = None;
                self.since = Some(now);
                true
            }
            _ => false,
        }
    }
    pub fn poll_animation(&mut self, now: Instant) -> bool {
        let Some(result) = self.animation.as_mut().and_then(|a| a.poll(now)) else {
            return false;
        };
        self.loaded = self.wanted.clone();
        match result {
            Ok(Some(Payload::Image {
                rgba,
                width,
                height,
            })) => match self.cache_image(&rgba, width, height) {
                Some(cache) => self.last = Some(cache),
                None => {
                    self.fail_resources();
                }
            },
            Ok(Some(Payload::Message(message))) => {
                self.terminal = true;
                self.last = Some(ViewCache::Message(message));
            }
            Ok(None) => {}
            _ => {
                self.terminal = true;
                self.last = Some(ViewCache::Message(self.preview_msgs.reader_stopped.clone()));
            }
        }
        true
    }

    /// Ocultar libera contenido/escena y cancela trabajo. Conserva identidad y el estado
    /// terminal: volver a mostrar no reintenta un error/cancelación por su cuenta.
    pub fn set_visible(&mut self, visible: bool, now: Instant) {
        if self.visible == visible {
            return;
        }
        if !visible {
            let terminal = self.terminal;
            self.cancel();
            self.terminal = terminal;
            self.loaded = None;
            if !terminal {
                self.last = None;
            }
            self.mesh_scene = None;
        }
        self.visible = visible;
        self.since = Some(now);
    }
}

impl Drop for PreviewState {
    fn drop(&mut self) {
        if let Some(token) = &self.token {
            token.cancel();
        }
        if let Some(token) = &self.mesh_token {
            token.cancel();
        }
    }
}

fn mesh_error_payload(
    error: naygo_core::mesh_preview::MeshError,
    msgs: &PreviewMessages,
) -> Payload {
    match error {
        naygo_core::mesh_preview::MeshError::ReaderStopped => {
            Payload::Message(msgs.reader_stopped.clone())
        }
        naygo_core::mesh_preview::MeshError::Cancelled => Payload::Message(msgs.cancelled.clone()),
        naygo_core::mesh_preview::MeshError::TooLarge
        | naygo_core::mesh_preview::MeshError::TooManyTriangles => {
            Payload::Message(msgs.image_big.clone())
        }
        _ => Payload::Message(msgs.decode.clone()),
    }
}

/// Construye el payload leyendo/decodificando en el hilo del worker.
#[cfg(test)]
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
    if token.is_cancelled() {
        return Payload::Message(msgs.cancelled.clone());
    }
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
#[cfg(test)]
fn read_mesh(path: &Path, token: &CancellationToken, msgs: &PreviewMessages) -> Payload {
    read_mesh_with_quality(path, token, msgs, Quality::default())
}
fn read_mesh_with_quality(
    path: &Path,
    token: &CancellationToken,
    msgs: &PreviewMessages,
    quality: Quality,
) -> Payload {
    let (width, height) = quality.mesh_size();
    match naygo_core::mesh_preview::render_file(path, width, height, token) {
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
/// Límite de bytes leídos al cargar el índice ZIP y consultar headers locales.
const ARCHIVE_ZIP_MAX_READ_BYTES: u64 = 8 * 1024 * 1024;
/// El parser seekable de `zip` materializa el directorio central completo antes de exponer filas.
const ARCHIVE_ZIP_MAX_INDEX_ENTRIES: u64 = 10_000;
const ARCHIVE_ZIP_MAX_DIRECTORY_BYTES: u64 = 6 * 1024 * 1024;
const ZIP_EOCD_SEARCH_BYTES: u64 = 22 + u16::MAX as u64;
const ZIP64_EOCD_SEARCH_BYTES: u64 = 1024 * 1024;
/// TAR y TAR.GZ no pueden saltar una entrada comprimida sin recorrer su contenido.
const ARCHIVE_MAX_COMPRESSED_BYTES: u64 = 16 * 1024 * 1024;
const ARCHIVE_MAX_UNCOMPRESSED_BYTES: u64 = 32 * 1024 * 1024;
/// Evita árboles patológicos por nombres y rutas internas de profundidad extrema.
const ARCHIVE_MAX_PATH_BYTES: usize = 1024;
const ARCHIVE_MAX_PATH_COMPONENTS: usize = 64;

/// Presupuesto compartido de lectura. Nunca entrega más de `limit` bytes y marca si el lector
/// pidió continuar después del límite; se usa tanto sobre ZIP seekable como sobre TAR/GZIP.
struct ArchiveReadBudget<R> {
    inner: R,
    remaining: u64,
    limit_hit: Arc<AtomicBool>,
}

impl<R> ArchiveReadBudget<R> {
    fn new(inner: R, limit: u64, limit_hit: Arc<AtomicBool>) -> Self {
        Self {
            inner,
            remaining: limit,
            limit_hit,
        }
    }
}

impl<R: std::io::Read> std::io::Read for ArchiveReadBudget<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            self.limit_hit.store(true, Ordering::Relaxed);
            return Ok(0);
        }
        let allowed = usize::try_from(self.remaining.min(buf.len() as u64)).unwrap_or(buf.len());
        let read = self.inner.read(&mut buf[..allowed])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}

impl<R: std::io::Seek> std::io::Seek for ArchiveReadBudget<R> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

#[derive(Clone, Copy)]
struct ZipIndexBounds {
    entries: u64,
    directory_bytes: u64,
}

/// Inspecciona sólo los footers acotados de ZIP32/ZIP64 antes de entregar el archivo a `zip`.
/// La crate construye un Vec con TODO el índice central; validar sus dimensiones evita reservar
/// memoria según contadores hostiles antes de que el presupuesto de lectura pueda actuar.
fn zip_index_bounds<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
) -> std::io::Result<Option<ZipIndexBounds>> {
    use std::io::SeekFrom;

    let file_len = reader.seek(SeekFrom::End(0))?;
    let tail_len = file_len.min(ZIP_EOCD_SEARCH_BYTES);
    if tail_len < 22 {
        return Ok(None);
    }
    reader.seek(SeekFrom::Start(file_len - tail_len))?;
    let mut tail = vec![0; tail_len as usize];
    reader.read_exact(&mut tail)?;
    let Some(eocd) = tail
        .windows(4)
        .enumerate()
        .rev()
        .find_map(|(offset, bytes)| {
            let fixed_end = offset.checked_add(22)?;
            if bytes != b"PK\x05\x06" || fixed_end > tail.len() {
                return None;
            }
            let comment_bytes = u16::from_le_bytes([tail[offset + 20], tail[offset + 21]]) as usize;
            (fixed_end.checked_add(comment_bytes)? <= tail.len()).then_some(offset)
        })
    else {
        return Ok(None);
    };
    let entries16 = u16::from_le_bytes([tail[eocd + 10], tail[eocd + 11]]);
    let directory_bytes32 = u32::from_le_bytes(
        tail[eocd + 12..eocd + 16]
            .try_into()
            .expect("EOCD fixed-size field"),
    );
    let needs_zip64 = entries16 == u16::MAX || directory_bytes32 == u32::MAX;
    if !needs_zip64 {
        return Ok(Some(ZipIndexBounds {
            entries: u64::from(entries16),
            directory_bytes: u64::from(directory_bytes32),
        }));
    }

    let eocd_absolute = file_len - tail_len + eocd as u64;
    let Some(locator_absolute) = eocd_absolute.checked_sub(20) else {
        return Ok(Some(ZipIndexBounds {
            entries: u64::MAX,
            directory_bytes: u64::MAX,
        }));
    };
    reader.seek(SeekFrom::Start(locator_absolute))?;
    let mut locator = [0; 20];
    reader.read_exact(&mut locator)?;
    if &locator[..4] != b"PK\x06\x07" {
        return Ok(Some(ZipIndexBounds {
            entries: u64::MAX,
            directory_bytes: u64::MAX,
        }));
    }

    // ZIP64 permite un sector extensible; se limita la búsqueda para que el propio preflight
    // también tenga un presupuesto y las variantes anormalmente grandes fallen con aviso.
    let search_start = locator_absolute.saturating_sub(ZIP64_EOCD_SEARCH_BYTES);
    reader.seek(SeekFrom::Start(search_start))?;
    let mut zip64_region = vec![0; (locator_absolute - search_start) as usize];
    reader.read_exact(&mut zip64_region)?;
    for offset in zip64_region
        .windows(4)
        .enumerate()
        .rev()
        .filter_map(|(offset, bytes)| (bytes == b"PK\x06\x06").then_some(offset))
    {
        if offset + 56 > zip64_region.len() {
            continue;
        }
        let record_bytes = u64::from_le_bytes(
            zip64_region[offset + 4..offset + 12]
                .try_into()
                .expect("ZIP64 EOCD record size"),
        );
        let Some(record_end) = (offset as u64)
            .checked_add(12)
            .and_then(|start| start.checked_add(record_bytes))
        else {
            continue;
        };
        if record_end != zip64_region.len() as u64 {
            continue;
        }
        let entries = u64::from_le_bytes(
            zip64_region[offset + 32..offset + 40]
                .try_into()
                .expect("ZIP64 entry count"),
        );
        let directory_bytes = u64::from_le_bytes(
            zip64_region[offset + 40..offset + 48]
                .try_into()
                .expect("ZIP64 directory size"),
        );
        return Ok(Some(ZipIndexBounds {
            entries,
            directory_bytes,
        }));
    }
    Ok(Some(ZipIndexBounds {
        entries: u64::MAX,
        directory_bytes: u64::MAX,
    }))
}

fn archive_path_is_bounded(path: &str) -> bool {
    path.len() <= ARCHIVE_MAX_PATH_BYTES
        && path
            .split('/')
            .filter(|part| !part.is_empty())
            .take(ARCHIVE_MAX_PATH_COMPONENTS + 1)
            .count()
            <= ARCHIVE_MAX_PATH_COMPONENTS
}

/// Suma tamaños hostiles sin permitir wraparound; ante overflow, el caller omite esa fila y
/// señala el árbol como parcial para no presentar un total falso.
fn add_archive_size(summary: &mut naygo_core::archive_tree::ArchiveSummary, size: u64) -> bool {
    let Some(total) = summary.total_uncompressed.checked_add(size) else {
        summary.truncated = true;
        return false;
    };
    summary.total_uncompressed = total;
    true
}

/// Lee el índice de un comprimido (zip/tar/tar.gz) y devuelve el texto del preview (encabezado
/// + árbol ASCII) vía `naygo_core::archive_tree`. Tolerante: ilegible/corrupto → `Payload::Message`.
///
/// Nunca escribe contenido. ZIP lee un índice acotado; TAR/GZIP avanzan por un flujo con límites
/// de bytes comprimidos/expandidos. `labels` ya llega traducido desde UI.
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
    let mut limit_hit = false;
    let read_ok = match fmt {
        ArchiveFormat::Zip => read_zip_entries(path, &mut entries, &mut summary, &mut limit_hit),
        ArchiveFormat::Tar => {
            read_tar_entries(path, &mut entries, &mut summary, false, &mut limit_hit)
        }
        ArchiveFormat::TarGz => {
            read_tar_entries(path, &mut entries, &mut summary, true, &mut limit_hit)
        }
    };
    if !read_ok {
        return Payload::Message(if limit_hit {
            msgs.reader_stopped.clone()
        } else {
            msgs.archive_bad.clone()
        });
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
        bytes_read: None,
        highlighted: None,
    }
}

/// Lee las entradas de un .zip. false si no se pudo abrir/leer.
fn read_zip_entries(
    path: &Path,
    entries: &mut Vec<naygo_core::archive_tree::ArchiveEntry>,
    summary: &mut naygo_core::archive_tree::ArchiveSummary,
    limit_hit: &mut bool,
) -> bool {
    use naygo_core::archive_tree::ArchiveEntry;
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let read_budget_hit = Arc::new(AtomicBool::new(false));
    let mut reader =
        ArchiveReadBudget::new(file, ARCHIVE_ZIP_MAX_READ_BYTES, read_budget_hit.clone());
    let index_bounds = match zip_index_bounds(&mut reader) {
        Ok(Some(bounds)) => Some(bounds),
        Ok(None) => None,
        Err(_) if read_budget_hit.load(Ordering::Relaxed) => {
            *limit_hit = true;
            return false;
        }
        Err(_) => None,
    };
    if index_bounds.is_some_and(|bounds| {
        bounds.entries > ARCHIVE_ZIP_MAX_INDEX_ENTRIES
            || bounds.directory_bytes > ARCHIVE_ZIP_MAX_DIRECTORY_BYTES
    }) {
        *limit_hit = true;
        return false;
    }
    let Ok(mut archive) = zip::ZipArchive::new(reader) else {
        *limit_hit = read_budget_hit.load(Ordering::Relaxed);
        return false;
    };
    let total = archive.len();
    summary.total_entries = total;
    let shown = total.min(ARCHIVE_MAX_ENTRIES);
    summary.truncated = total > shown;
    for i in 0..shown {
        let Ok(entry) = archive.by_index(i) else {
            summary.truncated = true;
            *limit_hit = read_budget_hit.load(Ordering::Relaxed);
            if *limit_hit {
                break;
            }
            continue;
        };
        let is_dir = entry.is_dir();
        let size = entry.size();
        let path = entry.name().replace('\\', "/");
        if !archive_path_is_bounded(&path) {
            summary.truncated = true;
            continue;
        }
        if is_dir {
            summary.dirs += 1;
        } else {
            if !add_archive_size(summary, size) {
                break;
            }
            summary.files += 1;
        }
        entries.push(ArchiveEntry { path, is_dir, size });
    }
    *limit_hit |= read_budget_hit.load(Ordering::Relaxed);
    if *limit_hit && entries.is_empty() {
        return false;
    }
    summary.truncated |= *limit_hit;
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
    limit_hit: &mut bool,
) -> bool {
    use naygo_core::archive_tree::ArchiveEntry;
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let compressed_limit_hit = Arc::new(AtomicBool::new(false));
    let uncompressed_limit_hit = Arc::new(AtomicBool::new(false));
    let reader: Box<dyn std::io::Read> = if gz {
        let compressed = ArchiveReadBudget::new(
            file,
            ARCHIVE_MAX_COMPRESSED_BYTES,
            compressed_limit_hit.clone(),
        );
        let decoder = flate2::read::GzDecoder::new(compressed);
        Box::new(ArchiveReadBudget::new(
            decoder,
            ARCHIVE_MAX_UNCOMPRESSED_BYTES,
            uncompressed_limit_hit.clone(),
        ))
    } else {
        Box::new(ArchiveReadBudget::new(
            file,
            ARCHIVE_MAX_UNCOMPRESSED_BYTES,
            uncompressed_limit_hit.clone(),
        ))
    };
    let mut had_error = false;
    let mut inspected = 0;
    {
        let mut archive = tar::Archive::new(reader);
        let Ok(iter) = archive.entries() else {
            *limit_hit = compressed_limit_hit.load(Ordering::Relaxed)
                || uncompressed_limit_hit.load(Ordering::Relaxed);
            return false;
        };
        for entry in iter {
            if inspected >= ARCHIVE_MAX_ENTRIES {
                summary.truncated = true;
                break;
            }
            inspected += 1;
            let e = match entry {
                Ok(e) => e,
                Err(_) => {
                    had_error = true;
                    if compressed_limit_hit.load(Ordering::Relaxed)
                        || uncompressed_limit_hit.load(Ordering::Relaxed)
                    {
                        break;
                    }
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
            if path_str.is_empty() || !archive_path_is_bounded(&path_str) {
                summary.truncated = true;
                continue;
            }
            if is_dir {
                summary.dirs += 1;
            } else {
                if !add_archive_size(summary, size) {
                    break;
                }
                summary.files += 1;
            }
            entries.push(ArchiveEntry {
                path: path_str,
                is_dir,
                size,
            });
        }
    }
    *limit_hit = compressed_limit_hit.load(Ordering::Relaxed)
        || uncompressed_limit_hit.load(Ordering::Relaxed);
    summary.truncated |= *limit_hit;
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
    if *limit_hit && entries.is_empty() {
        return false;
    }
    true
}

/// Lee el archivo como texto y, si `lang` está presente (la regla fuerza un lenguaje), resalta
/// el texto YA recortado con `core::highlight` (defensa i16 intacta: se resalta lo recortado).
#[cfg(test)]
fn read_text(path: &Path, lang: Option<CodeLang>, msgs: &PreviewMessages) -> Payload {
    read_text_with_limit(path, lang, msgs, preview::DEFAULT_TEXT_MAX_BYTES as usize)
}

fn read_text_with_limit(
    path: &Path,
    lang: Option<CodeLang>,
    msgs: &PreviewMessages,
    max_bytes: usize,
) -> Payload {
    use naygo_core::preview::truncate_text;
    use std::io::Read;
    let max_bytes =
        preview::normalize_text_max_bytes(max_bytes.min(preview::TEXT_MAX_BYTES) as u32) as usize;
    let Ok(mut file) = std::fs::File::open(path) else {
        return Payload::Message(msgs.read.clone());
    };
    let mut buf = Vec::with_capacity(max_bytes.min(8192));
    let mut chunk = [0u8; 8192];
    let mut hit_cap = false;
    loop {
        match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() >= max_bytes {
                    buf.truncate(max_bytes);
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
        bytes_read: Some(buf.len()),
        highlighted,
    }
}

#[cfg(test)]
fn read_image(path: &Path, token: &CancellationToken, msgs: &PreviewMessages) -> Payload {
    read_image_with_quality(path, token, msgs, Quality::default())
}
fn read_image_with_quality(
    path: &Path,
    token: &CancellationToken,
    msgs: &PreviewMessages,
    quality: Quality,
) -> Payload {
    use naygo_core::preview::IMAGE_MAX_BYTES;
    match std::fs::metadata(path) {
        Ok(m) if m.len() > IMAGE_MAX_BYTES => return Payload::Message(msgs.image_big.clone()),
        Ok(_) => {}
        Err(_) => return Payload::Message(msgs.read.clone()),
    }
    if token.is_cancelled() {
        return Payload::Message(msgs.cancelled.clone());
    }
    let bytes = match read_bounded(path, IMAGE_MAX_BYTES) {
        Ok(bytes) => bytes,
        Err(_) => return Payload::Message(msgs.image_big.clone()),
    };
    let Ok(mut reader) = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()
    else {
        return Payload::Message(msgs.decode.clone());
    };
    // Decoder limits are defense in depth, NOT a claim about total process memory.
    // The Windows Job is the hard committed-memory backstop for non-cooperative codecs.
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let img = match reader.decode() {
        Ok(img) if u64::from(img.width()) * u64::from(img.height()) <= 16 * 1024 * 1024 => img,
        Ok(_) | Err(image::ImageError::Limits(_)) => {
            return Payload::Message(msgs.image_big.clone())
        }
        Err(_) => return Payload::Message(msgs.decode.clone()),
    };
    if token.is_cancelled() {
        return Payload::Message(msgs.cancelled.clone());
    }
    let (w, h) = (img.width(), img.height());
    let side = quality.image_side();
    let scaled = if w > side || h > side {
        img.thumbnail(side, side)
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
/// quede en `IMAGE_MAX_SIDE` px (nítido pero acotado). Un SVG sin `viewBox` ni dimensiones
/// explícitas usa los límites reales de su contenido como canvas: varios assets exportados desde
/// Flash/Animate usan coordenadas negativas y, con el canvas SVG implícito de 100×100, quedaban
/// recortados en la vista previa. Como el SVG es vectorial, se respeta el tope de bytes del
/// ARCHIVO fuente (no del bitmap resultante). Cancelable entre etapas.
fn read_bounded(path: &Path, maximum: u64) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(std::io::Error::other("preview input limit"));
    }
    Ok(bytes)
}

#[cfg(test)]
fn read_svg(path: &Path, token: &CancellationToken, msgs: &PreviewMessages) -> Payload {
    read_svg_with_quality(path, token, msgs, Quality::default())
}
fn read_svg_with_quality(
    path: &Path,
    token: &CancellationToken,
    msgs: &PreviewMessages,
    quality: Quality,
) -> Payload {
    use naygo_core::preview::IMAGE_MAX_BYTES;
    let bytes = match std::fs::metadata(path) {
        Ok(m) if m.len() > IMAGE_MAX_BYTES => return Payload::Message(msgs.svg_big.clone()),
        Ok(_) => match read_bounded(path, IMAGE_MAX_BYTES) {
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
    // Embedded data images remain available; arbitrary external file/UNC references are
    // not part of a self-contained preview and must never trigger extra filesystem reads.
    let mut opt = usvg::Options::default();
    opt.image_href_resolver.resolve_string = Box::new(|_, _| None);
    let tree = match usvg::Tree::from_data(&bytes, &opt) {
        Ok(t) => t,
        Err(_) => return Payload::Message(msgs.svg_bad.clone()),
    };
    if token.is_cancelled() {
        return Payload::Message(msgs.cancelled.clone());
    }
    let canvas = svg_canvas(&tree, &bytes);
    // Un SVG es vectorial: incluso si declara 64×64, rasterizarlo a la resolución de preview
    // evita que Slint deba ampliar un bitmap chico y produzca bordes pixelados. El lado mayor
    // sigue estrictamente acotado por IMAGE_MAX_SIDE.
    let (sw, sh) = (canvas.width, canvas.height);
    let longest = sw.max(sh).max(1.0);
    if !longest.is_finite() || !sw.is_finite() || !sh.is_finite() {
        return Payload::Message(msgs.svg_big.clone());
    }
    let side = quality.image_side() as f32;
    let scale = side / longest;
    let pw = (sw * scale).ceil().clamp(1.0, side) as u32;
    let ph = (sh * scale).ceil().clamp(1.0, side) as u32;
    let mut pixmap = match tiny_skia::Pixmap::new(pw, ph) {
        Some(p) => p,
        None => return Payload::Message(msgs.rasterize.clone()),
    };
    // Primero trasladamos el contenido al origen (necesario para SVGs sin canvas explícito) y
    // luego lo escalamos. `pre_translate` conserva justamente ese orden en tiny-skia.
    let transform =
        tiny_skia::Transform::from_scale(scale, scale).pre_translate(-canvas.x, -canvas.y);
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

#[derive(Clone, Copy, Debug)]
struct SvgCanvas {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

/// Selecciona el canvas de render. Los SVG con `viewBox` o dimensiones declaradas conservan su
/// espacio de diseño (incluido cualquier margen intencional). Si faltan ambos, usvg debe aplicar
/// el viewport HTML por defecto aunque el dibujo esté en coordenadas negativas; allí encuadramos
/// el contenido real para no recortarlo.
fn svg_canvas(tree: &usvg::Tree, bytes: &[u8]) -> SvgCanvas {
    if svg_has_explicit_canvas(bytes) {
        let size = tree.size();
        return SvgCanvas {
            x: 0.0,
            y: 0.0,
            width: size.width().max(1.0),
            height: size.height().max(1.0),
        };
    }

    let bounds = tree.root().abs_layer_bounding_box();
    SvgCanvas {
        x: bounds.x(),
        y: bounds.y(),
        width: bounds.width().max(1.0),
        height: bounds.height().max(1.0),
    }
}

/// Lee solo la etiqueta raíz. No pretendemos reemplazar un parser XML: usvg ya valida el SVG;
/// este chequeo decide si debe respetarse su canvas o corregirse el viewport implícito.
fn svg_has_explicit_canvas(bytes: &[u8]) -> bool {
    // Attribute names are ASCII. Inspect only the root tag so a 20 MiB SVG does not need a
    // lossy String and a second full-size lowercase copy just to answer this question.
    let Some(start) = bytes
        .windows(b"<svg".len())
        .position(|window| window.eq_ignore_ascii_case(b"<svg"))
    else {
        return true;
    };
    let Some(end_rel) = bytes[start..].iter().position(|byte| *byte == b'>') else {
        return true;
    };
    let tag = &bytes[start..start + end_rel];
    let has = |attribute: &[u8]| {
        tag.windows(attribute.len())
            .any(|window| window.eq_ignore_ascii_case(attribute))
    };
    has(b"viewbox") || (has(b"width") && has(b"height"))
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
/// Tope de páginas cuyo contenido se recorre; `get_pages()` sigue contando el total para el título.
const PDF_MAX_PAGES: usize = 500;

/// Writer acotado para cortar la extracción dentro de `pdf-extract`, antes de construir un
/// `String` completo para documentos que contienen muchísimo texto. El parser corre en el proceso
/// aislado de preview y termina cuando recibe `fmt::Error` al alcanzar el máximo.
#[derive(Debug)]
struct BoundedPdfText {
    text: String,
    chars: usize,
    max_chars: usize,
    truncated: bool,
}

impl BoundedPdfText {
    fn new(max_chars: usize) -> Self {
        Self {
            text: String::with_capacity(max_chars.min(8192)),
            chars: 0,
            max_chars,
            truncated: false,
        }
    }
}

impl std::fmt::Write for BoundedPdfText {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let mut chars = value.chars();
        let remaining = self.max_chars.saturating_sub(self.chars);
        for _ in 0..remaining {
            let Some(ch) = chars.next() else {
                return Ok(());
            };
            self.text.push(ch);
            self.chars += 1;
        }
        if chars.next().is_some() {
            self.truncated = true;
            Err(std::fmt::Error)
        } else {
            Ok(())
        }
    }
}

impl pdf_extract::ConvertToFmt for &mut BoundedPdfText {
    type Writer = Self;

    fn convert(self) -> Self::Writer {
        self
    }
}

/// Limita las páginas de contenido procesadas, sin cambiar el algoritmo de extracción de texto
/// que ofrece `pdf-extract` para cada página.
struct BoundedPdfOutput<'a> {
    inner: pdf_extract::PlainTextOutput<&'a mut BoundedPdfText>,
    pages: usize,
    max_pages: usize,
    page_limit_reached: bool,
}

impl<'a> BoundedPdfOutput<'a> {
    fn new(text: &'a mut BoundedPdfText, max_pages: usize) -> Self {
        Self {
            inner: pdf_extract::PlainTextOutput::new(text),
            pages: 0,
            max_pages,
            page_limit_reached: false,
        }
    }
}

impl pdf_extract::OutputDev for BoundedPdfOutput<'_> {
    fn begin_page(
        &mut self,
        page_num: u32,
        media_box: &pdf_extract::MediaBox,
        art_box: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), pdf_extract::OutputError> {
        if self.pages >= self.max_pages {
            self.page_limit_reached = true;
            return Err(pdf_extract::OutputError::FormatError(std::fmt::Error));
        }
        self.pages += 1;
        self.inner.begin_page(page_num, media_box, art_box)
    }

    fn end_page(&mut self) -> Result<(), pdf_extract::OutputError> {
        self.inner.end_page()
    }

    fn output_character(
        &mut self,
        transform: &pdf_extract::Transform,
        width: f64,
        spacing: f64,
        font_size: f64,
        character: &str,
    ) -> Result<(), pdf_extract::OutputError> {
        self.inner
            .output_character(transform, width, spacing, font_size, character)
    }

    fn begin_word(&mut self) -> Result<(), pdf_extract::OutputError> {
        self.inner.begin_word()
    }

    fn end_word(&mut self) -> Result<(), pdf_extract::OutputError> {
        self.inner.end_word()
    }

    fn end_line(&mut self) -> Result<(), pdf_extract::OutputError> {
        self.inner.end_line()
    }

    fn stroke(
        &mut self,
        transform: &pdf_extract::Transform,
        colorspace: &pdf_extract::ColorSpace,
        color: &[f64],
        path: &pdf_extract::Path,
    ) -> Result<(), pdf_extract::OutputError> {
        self.inner.stroke(transform, colorspace, color, path)
    }

    fn fill(
        &mut self,
        transform: &pdf_extract::Transform,
        colorspace: &pdf_extract::ColorSpace,
        color: &[f64],
        path: &pdf_extract::Path,
    ) -> Result<(), pdf_extract::OutputError> {
        self.inner.fill(transform, colorspace, color, path)
    }
}

/// Preview LIVIANO de un PDF: extrae el texto (puro Rust, sin DLLs) y un encabezado con el nº de
/// páginas. NO renderiza la página (eso requeriría una DLL nativa). Devuelve `Payload::Text`.
fn read_pdf(path: &Path, msgs: &PreviewMessages) -> Payload {
    match std::fs::metadata(path) {
        Ok(m) if m.len() > PDF_MAX_BYTES => return Payload::Message(msgs.pdf_big.clone()),
        Ok(_) => {}
        Err(_) => return Payload::Message(msgs.read.clone()),
    }
    // `pdf-extract` reexporta el `Document` que usa internamente (lopdf 0.38). Leer las páginas
    // y extraer texto desde una sola carga conserva el mismo parser y formato de PlainTextOutput.
    // La metadata general continúa usando lopdf 0.41 desde naygo-core.
    let mut doc =
        match crate::logging::catch_recoverable_panic(|| pdf_extract::Document::load(path)) {
            Ok(Ok(doc)) => doc,
            Ok(Err(_)) | Err(_) => {
                return Payload::Text {
                    text: msgs.pdf_no_text.clone(),
                    truncated: false,
                    bytes_read: None,
                    highlighted: None,
                };
            }
        };
    let pages = Some(doc.get_pages().len());
    // Frase traducida con el nº de páginas (placeholder `{n}`). El código añade la puntuación
    // final (`.` sin texto, `:` con texto) y los saltos; así una sola clave i18n cubre ambas.
    let pages_phrase = |n: usize| msgs.pdf_pages.replace("{n}", &n.to_string());
    // `pdf-extract` puede hacer panic ante streams sintácticamente válidos pero con operandos
    // faltantes. El writer se detiene al alcanzar el máximo, antes de acumular texto ilimitado.
    // El proceso aislado conserva su límite de memoria, timeout y cancelación.
    let extracted = crate::logging::catch_recoverable_panic(|| {
        let mut bounded = BoundedPdfText::new(PDF_TEXT_MAX_CHARS);
        let (result, page_limit_reached) = {
            let mut output = BoundedPdfOutput::new(&mut bounded, PDF_MAX_PAGES);
            let result = if doc.is_encrypted() {
                pdf_extract::output_doc_encrypted(&mut doc, &mut output, "")
            } else {
                pdf_extract::output_doc(&doc, &mut output)
            };
            (result, output.page_limit_reached)
        };
        (result, bounded, page_limit_reached)
    });
    let (text, truncated) = match extracted {
        Ok((result, bounded, page_limit_reached))
            if result.is_ok() || bounded.truncated || page_limit_reached =>
        {
            (bounded.text, bounded.truncated || page_limit_reached)
        }
        Ok(_) | Err(_) => {
            let head = match pages {
                Some(n) => format!("{}.\n\n", pages_phrase(n)),
                None => String::new(),
            };
            return Payload::Text {
                text: format!("{head}{}", msgs.pdf_no_text),
                truncated: false,
                bytes_read: None,
                highlighted: None,
            };
        }
    };
    let header = match pages {
        Some(n) => format!("{}:\n\n", pages_phrase(n)),
        None => "PDF:\n\n".to_string(),
    };
    let trimmed = text.trim();
    // Recorte por línea: el texto extraído de un PDF puede traer líneas larguísimas que
    // desbordarían el render por software (glifo fuera del rango i16). Misma defensa que el
    // preview de texto.
    let (body, clipped) = naygo_core::preview::clip_long_lines(trimmed);
    Payload::Text {
        text: format!("{header}{body}"),
        truncated: truncated || clipped,
        bytes_read: None,
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
    fn preflight_zip32_lee_cantidad_y_tamano_del_indice() {
        use std::io::Cursor;

        let mut eocd = vec![0u8; 22];
        eocd[..4].copy_from_slice(b"PK\x05\x06");
        eocd[8..10].copy_from_slice(&7u16.to_le_bytes());
        eocd[10..12].copy_from_slice(&7u16.to_le_bytes());
        eocd[12..16].copy_from_slice(&512u32.to_le_bytes());
        let bounds = zip_index_bounds(&mut Cursor::new(eocd))
            .unwrap()
            .expect("footer ZIP32");
        assert_eq!(bounds.entries, 7);
        assert_eq!(bounds.directory_bytes, 512);
    }

    #[test]
    fn preflight_zip64_lee_contadores_de_64_bits() {
        use std::io::Cursor;

        let mut bytes = vec![0u8; 56 + 20 + 22];
        bytes[..4].copy_from_slice(b"PK\x06\x06");
        bytes[4..12].copy_from_slice(&44u64.to_le_bytes());
        bytes[24..32].copy_from_slice(&10_001u64.to_le_bytes());
        bytes[32..40].copy_from_slice(&10_001u64.to_le_bytes());
        bytes[40..48].copy_from_slice(&700_000u64.to_le_bytes());
        let locator = 56;
        bytes[locator..locator + 4].copy_from_slice(b"PK\x06\x07");
        bytes[locator + 8..locator + 16].copy_from_slice(&0u64.to_le_bytes());
        let eocd = locator + 20;
        bytes[eocd..eocd + 4].copy_from_slice(b"PK\x05\x06");
        bytes[eocd + 10..eocd + 12].copy_from_slice(&u16::MAX.to_le_bytes());
        bytes[eocd + 12..eocd + 16].copy_from_slice(&u32::MAX.to_le_bytes());

        let bounds = zip_index_bounds(&mut Cursor::new(bytes))
            .unwrap()
            .expect("footer ZIP64");
        assert_eq!(bounds.entries, 10_001);
        assert_eq!(bounds.directory_bytes, 700_000);
        assert!(bounds.entries > ARCHIVE_ZIP_MAX_INDEX_ENTRIES);
    }

    #[test]
    fn presupuesto_de_archivo_limita_bytes_y_registra_intento_posterior() {
        use std::io::Read as _;

        let hit = Arc::new(AtomicBool::new(false));
        let mut reader = ArchiveReadBudget::new(std::io::Cursor::new(vec![7; 8]), 5, hit.clone());
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, vec![7; 5]);
        assert!(hit.load(Ordering::Relaxed));
    }

    #[test]
    fn ruta_de_archivo_limita_largo_y_profundidad() {
        assert!(archive_path_is_bounded("carpeta/archivo.txt"));
        assert!(!archive_path_is_bounded(
            &"x".repeat(ARCHIVE_MAX_PATH_BYTES + 1)
        ));
        let deep = format!(
            "{}/file",
            vec!["x"; ARCHIVE_MAX_PATH_COMPONENTS + 1].join("/")
        );
        assert!(!archive_path_is_bounded(&deep));
    }

    #[test]
    fn suma_de_tamano_de_archivo_no_desborda() {
        let mut summary = naygo_core::archive_tree::ArchiveSummary {
            total_uncompressed: u64::MAX,
            ..Default::default()
        };
        assert!(!add_archive_size(&mut summary, 1));
        assert_eq!(summary.total_uncompressed, u64::MAX);
        assert!(summary.truncated);
    }

    #[test]
    fn svg_sin_canvas_expone_todo_su_contenido() {
        // Export típico de Kenney/Flash: coordenadas negativas, sin width/height ni viewBox.
        // El viewport implícito de SVG lo recortaría; el preview debe encuadrar el dibujo entero.
        let source = br##"<svg xmlns="http://www.w3.org/2000/svg"><path fill="#fff" d="M-64 -32H64V32H-64Z"/></svg>"##;
        let tree = usvg::Tree::from_data(source, &usvg::Options::default()).unwrap();
        let canvas = svg_canvas(&tree, source);
        assert!(canvas.x <= -64.0);
        assert!(canvas.y <= -32.0);
        assert!(canvas.width >= 128.0);
        assert!(canvas.height >= 64.0);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sin-canvas.svg");
        std::fs::write(&path, source).unwrap();
        match read_svg(&path, &CancellationToken::new(), &msgs_es()) {
            Payload::Image {
                width,
                height,
                rgba,
            } => {
                assert_eq!((width, height), (2048, 1024));
                assert_eq!(
                    rgba[3], 255,
                    "el dibujo llega al borde del canvas corregido"
                );
            }
            other => panic!("esperaba SVG rasterizado, fue {other:?}"),
        }
    }

    #[test]
    fn svg_con_viewbox_conserva_su_canvas_declarado() {
        let source = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 180"><path d="M20 20H40V40H20Z"/></svg>"#;
        let tree = usvg::Tree::from_data(source, &usvg::Options::default()).unwrap();
        let canvas = svg_canvas(&tree, source);
        assert_eq!(canvas.x, 0.0);
        assert_eq!(canvas.y, 0.0);
        assert_eq!(canvas.width, 320.0);
        assert_eq!(canvas.height, 180.0);
    }

    #[test]
    fn canvas_svg_detecta_atributos_ascii_sin_convertir_el_documento() {
        assert!(svg_has_explicit_canvas(
            br#"<SVG WIDTH="320" HEIGHT="180">"#
        ));
        assert!(svg_has_explicit_canvas(br#"<svg viewBox="0 0 320 180">"#));
        assert!(!svg_has_explicit_canvas(
            br#"<svg xmlns="http://www.w3.org/2000/svg">"#
        ));
    }

    #[test]
    fn svg_con_muchos_elementos_se_rasteriza_a_dimensiones_acotadas() {
        let mut source =
            String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">"#);
        for index in 0..4_000 {
            let x = index % 64;
            let y = (index / 64) % 64;
            source.push_str(&format!(r#"<circle cx="{x}" cy="{y}" r="1"/>"#));
        }
        source.push_str("</svg>");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("muchos-elementos.svg");
        std::fs::write(&path, source).unwrap();

        match read_svg(&path, &CancellationToken::new(), &msgs_es()) {
            Payload::Image {
                width,
                height,
                rgba,
            } => {
                assert_eq!((width, height), (2048, 2048));
                assert_eq!(rgba.len(), 2048 * 2048 * 4);
            }
            other => panic!("esperaba SVG rasterizado, fue {other:?}"),
        }
    }

    #[test]
    fn imagen_con_ancho_mayor_al_limite_se_rechaza_antes_de_escalar() {
        let width = 16_385;
        let image = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(
            width,
            1,
            vec![0; width as usize * 4],
        )
        .unwrap();
        let mut encoded = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ancho-extremo.png");
        std::fs::write(&path, encoded.into_inner()).unwrap();

        assert!(matches!(
            read_image(&path, &CancellationToken::new(), &msgs_es()),
            Payload::Message(message) if message == msgs_es().image_big
        ));
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
    fn writer_pdf_limita_caracteres_unicode_y_senala_el_corte() {
        use std::fmt::Write as _;

        let mut output = BoundedPdfText::new(4);
        assert!(write!(&mut output, "abá中de").is_err());
        assert_eq!(output.text, "abá中");
        assert_eq!(output.chars, 4);
        assert!(output.truncated);

        let mut exact = BoundedPdfText::new(4);
        assert!(write!(&mut exact, "abá中").is_ok());
        assert_eq!(exact.text, "abá中");
        assert!(!exact.truncated);
    }

    #[test]
    fn output_pdf_detiene_recorrido_al_limite_de_paginas() {
        use pdf_extract::OutputDev as _;

        let mut text = BoundedPdfText::new(PDF_TEXT_MAX_CHARS);
        let mut output = BoundedPdfOutput::new(&mut text, 2);
        let media_box = pdf_extract::MediaBox {
            llx: 0.0,
            lly: 0.0,
            urx: 612.0,
            ury: 792.0,
        };
        assert!(output.begin_page(1, &media_box, None).is_ok());
        assert!(output.begin_page(2, &media_box, None).is_ok());
        assert!(output.begin_page(3, &media_box, None).is_err());
        assert!(output.page_limit_reached);
    }

    #[test]
    fn pdf_muy_largo_se_corta_durante_la_extraccion() {
        use lopdf::{dictionary, Document, Stream};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("texto-extenso.pdf");
        let mut doc = Document::with_version("1.5");
        let pages = doc.new_object_id();
        let font = doc.add_object(
            dictionary! {"Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica"},
        );
        let content = format!(
            "BT /F1 12 Tf ({}) Tj ET",
            "A".repeat(PDF_TEXT_MAX_CHARS + 500)
        );
        let content = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => dictionary! {"Font" => dictionary! {"F1" => font}},
            "Contents" => content,
        });
        doc.objects.insert(
            pages,
            dictionary! {"Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1}.into(),
        );
        let catalog = doc.add_object(dictionary! {"Type" => "Catalog", "Pages" => pages});
        doc.trailer.set("Root", catalog);
        doc.save(&path).unwrap();

        let Payload::Text {
            text, truncated, ..
        } = read_pdf(&path, &msgs_es())
        else {
            panic!("se esperaba texto de preview PDF")
        };
        assert!(truncated, "la salida debe indicar el corte por límite");
        let (_, body) = text.split_once("\n\n").expect("encabezado y cuerpo");
        assert!(body.chars().count() <= PDF_TEXT_MAX_CHARS);
        assert!(body.chars().count() >= naygo_core::preview::TEXT_MAX_LINE_CHARS);
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
    fn cancelar_preview_se_ofrece_desde_el_debounce() {
        let mut s = PreviewState::new();
        let t0 = Instant::now();
        s.set_wanted(Some(PathBuf::from("C:/x/lento.stl")), t0);
        // Simula el instante en que ya arrancó un worker (el test no toca disco ni hilos).
        assert!(s.loading());
        assert!(s.can_cancel(t0));
        assert!(s.cancel());
        assert!(!s.busy());
        assert!(!s.should_start(t0 + PREVIEW_DEBOUNCE));
        assert!(s.retry(t0));
        assert!(s.should_start(t0 + PREVIEW_DEBOUNCE));
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
    fn zip_lista_hasta_el_limite_y_marca_entradas_restantes() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("muchas-entradas.zip");
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
            for index in 0..=ARCHIVE_MAX_ENTRIES {
                writer
                    .start_file(format!("archivo-{index:03}.txt"), options)
                    .unwrap();
            }
            writer.finish().unwrap();
        }

        let Payload::Text {
            text, truncated, ..
        } = read_archive_listing(&zip_path, &labels_es(), &msgs_es())
        else {
            panic!("se esperaba el listado ZIP parcial")
        };
        assert!(truncated);
        assert!(text.contains("archivo-000.txt"));
        assert!(!text.contains("archivo-500.txt"));
        assert!(text.contains("… y 1 más"));
    }

    #[test]
    fn zip_con_indice_mayor_al_presupuesto_se_detiene_con_mensaje_de_limite() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("indice-grande.zip");
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
            let prefix = "x".repeat(65_000);
            for index in 0..130 {
                writer
                    .start_file(format!("{prefix}-{index:03}.txt"), options)
                    .unwrap();
            }
            writer.finish().unwrap();
        }

        let payload = read_archive_listing(&zip_path, &labels_es(), &msgs_es());
        assert!(
            matches!(payload, Payload::Message(message) if message == msgs_es().reader_stopped)
        );
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
    fn targz_detiene_expansion_y_conserva_listado_parcial() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("expansion-grande.tar.gz");
        {
            let file = std::fs::File::create(&path).unwrap();
            let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
            let mut builder = tar::Builder::new(encoder);
            let data = vec![0u8; ARCHIVE_MAX_UNCOMPRESSED_BYTES as usize + 1];
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, "grande.bin", &data[..])
                .unwrap();
            let small = b"despues";
            let mut header = tar::Header::new_gnu();
            header.set_size(small.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, "despues.txt", &small[..])
                .unwrap();
            builder.into_inner().unwrap().finish().unwrap();
        }

        let Payload::Text {
            text, truncated, ..
        } = read_archive_listing(&path, &labels_es(), &msgs_es())
        else {
            panic!("se esperaba un listado TAR.GZ parcial")
        };
        assert!(truncated);
        assert!(text.contains("grande.bin"));
        assert!(!text.contains("despues.txt"));
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
    fn sif_automatico_se_previsualiza_como_texto() {
        // `.sif` en Auto debe seguir la clasificación textual nueva y no exigir ajuste manual.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("modelo.sif");
        std::fs::write(&p, b"linea de texto\n").unwrap();
        let rules = preview::default_preview_rules();
        let token = CancellationToken::new();
        match build_payload(&p, &rules, false, &labels_es(), &msgs_es(), &token) {
            Payload::Text { text, .. } => assert!(text.contains("linea de texto")),
            other => panic!("esperaba texto para .sif, fue {other:?}"),
        }
    }

    #[test]
    fn shell_script_textual_se_lee_con_modo_automatico() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ordenar.sh");
        std::fs::write(&path, b"#!/bin/sh\necho listo\n").unwrap();
        let rules = preview::default_preview_rules();
        let token = CancellationToken::new();

        match build_payload(&path, &rules, false, &labels_es(), &msgs_es(), &token) {
            Payload::Text { text, .. } => assert!(text.contains("echo listo")),
            other => panic!("esperaba texto para .sh, fue {other:?}"),
        }
    }

    #[test]
    fn reader_usa_el_limite_configurado_y_reporta_bytes_leidos() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grande.txt");
        std::fs::write(&path, "x\n".repeat(400_000)).unwrap();

        let payload = read_text_with_limit(&path, None, &msgs_es(), 512 * 1024);
        assert!(matches!(
            payload,
            Payload::Text {
                truncated: true,
                bytes_read: Some(bytes_read),
                ..
            } if bytes_read == 512 * 1024
        ));
    }
}
