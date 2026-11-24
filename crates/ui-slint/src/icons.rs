// Naygo — cache de íconos: PNG (embebido o pack suelto del usuario) → slint::Image, decodificado
// UNA sola vez. Clave del rendimiento: el render por software no re-rasteriza; clonar un
// slint::Image comparte el buffer. El set activo es un id (String), igual que en la capa egui:
// `flat`/`fluent`/`mono` resuelven a los assets embebidos; cualquier otro id es un pack suelto
// del usuario bajo `<data_dir>/icons/<id>/<name>.png` (6E), con lectura legacy desde config.
// Ver `naygo_core::icons`.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use naygo_core::icon_kind::IconKey;
use slint::{Image, SharedPixelBuffer};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, SyncSender};

/// Clave del cache: (set_activo, ícono, color_tinte, tintable).
// La clave NO incluye el set activo: `set_active` limpia el cache al cambiar de set, así el map
// solo contiene íconos del set vigente. Evita clonar el `String` del set en cada `get` (camino
// caliente, una vez por fila), a cambio de re-decodificar al cambiar de set (acción rara).
type CacheKey = (IconKey, (u8, u8, u8), bool);
type PickerCacheKey = (String, IconKey, (u8, u8, u8), bool);

#[derive(Clone)]
struct DecodedIcon {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[derive(Clone)]
enum IconLoadTarget {
    Active { generation: u64, key: IconKey },
    Picker(PickerCacheKey),
}

struct IconLoadRequest {
    target: IconLoadTarget,
    set_id: String,
    key: IconKey,
    overrides: std::collections::BTreeMap<String, naygo_core::icon_source::IconSource>,
    data_dir: PathBuf,
    legacy_config_dir: PathBuf,
    tintable: bool,
    tint: (u8, u8, u8),
    wake: Option<naygo_platform::dir_watch::Waker>,
}

struct IconLoadResult {
    target: IconLoadTarget,
    decoded: Option<DecodedIcon>,
}

/// Resultado del sondeo del worker de íconos. `changed` invalida las firmas de filas; los errores
/// son diagnósticos discretos y el fallback embebido sigue visible.
pub(crate) struct IconPump {
    pub changed: bool,
    pub errors: Vec<String>,
}

/// Decodifica y cachea los íconos a `slint::Image` por (set_id, clave). El set activo lo fija
/// la configuración (`Settings.icon_set`, un id de set). `data_dir` resuelve packs nuevos;
/// `legacy_config_dir` conserva la lectura de assets importados por versiones anteriores.
pub struct IconCache {
    control_map: HashMap<CacheKey, Image>,
    /// Clave: (set_activo, clave_ícono, color_tinte, tintable). Incluye el tinte para
    /// que al cambiar el color del tema se fuerce re-decodificación sin limpiar el cache.
    map: HashMap<CacheKey, Image>,
    /// Muestras por set en la galería/picker; no mezclarlas con el atlas del set activo.
    picker_map: HashMap<PickerCacheKey, Image>,
    /// Caché de íconos EXACTOS por extensión (capa global, sin tinte): ext → imagen.
    ft_map: HashMap<String, Image>,
    /// Íconos del Shell por extensión. None representa una asociación consultada sin resultado.
    system_file_icons: bool,
    system_file_map: HashMap<String, Option<Image>>,
    system_file_requested: HashSet<String>,
    system_file_refresh_pending: bool,
    system_file_tx: Option<SyncSender<(String, Option<naygo_platform::dir_watch::Waker>)>>,
    system_file_rx: Option<
        Receiver<(
            String,
            Option<naygo_platform::system_file_icons::SystemFileIcon>,
        )>,
    >,
    system_file_revision: u64,
    /// Id del set activo (el que devuelve `get`). La UI lo cambia con `set_active`.
    active: String,
    /// Directorio de datos del usuario: contiene los packs de íconos actuales.
    data_dir: PathBuf,
    /// Directorio de configuración histórico, solo de lectura para assets pre-L-02.
    legacy_config_dir: PathBuf,
    /// Overrides por nombre de archivo de ícono (p.ej. "folder") → fuente alternativa.
    overrides: std::collections::BTreeMap<String, naygo_core::icon_source::IconSource>,
    /// Color de tinte (RGB) cuando el set activo es tintable.
    tint: (u8, u8, u8),
    /// Si es `true`, el set activo es una máscara blanca que se tiñe con `tint`.
    tintable: bool,
    generation: u64,
    external_revision: u64,
    active_pending: HashSet<(u64, IconKey)>,
    active_failed: HashSet<(u64, IconKey)>,
    picker_pending: HashSet<PickerCacheKey>,
    picker_failed: HashSet<PickerCacheKey>,
    icon_tx: Option<SyncSender<IconLoadRequest>>,
    icon_rx: Receiver<IconLoadResult>,
    waker: Option<naygo_platform::dir_watch::Waker>,
}

impl IconCache {
    #[cfg(test)]
    pub fn new(active: impl Into<String>, config_dir: PathBuf) -> IconCache {
        Self::new_with_legacy(active, config_dir.clone(), config_dir)
    }

    pub fn new_with_legacy(
        active: impl Into<String>,
        data_dir: PathBuf,
        legacy_config_dir: PathBuf,
    ) -> IconCache {
        let (tx, request_rx) = std::sync::mpsc::sync_channel::<IconLoadRequest>(32);
        let (result_tx, result_rx) = std::sync::mpsc::channel::<IconLoadResult>();
        let worker_started = std::thread::Builder::new()
            .name("naygo-external-icons".into())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    let bytes = naygo_core::icons::resolve_with_overrides_from(
                        &request.set_id,
                        &request.overrides,
                        request.key,
                        &request.data_dir,
                        &request.legacy_config_dir,
                    );
                    let decoded = decode_external(&bytes, request.tintable, request.tint);
                    if result_tx
                        .send(IconLoadResult {
                            target: request.target,
                            decoded,
                        })
                        .is_err()
                    {
                        break;
                    }
                    // Si el event loop estaba dormido, despertar para aplicar el resultado.
                    if let Some(wake) = request.wake {
                        wake();
                    }
                }
            })
            .is_ok();
        IconCache {
            control_map: HashMap::new(),
            map: HashMap::new(),
            picker_map: HashMap::new(),
            ft_map: HashMap::new(),
            system_file_icons: false,
            system_file_map: HashMap::new(),
            system_file_requested: HashSet::new(),
            system_file_refresh_pending: false,
            system_file_tx: None,
            system_file_rx: None,
            system_file_revision: 0,
            active: active.into(),
            data_dir,
            legacy_config_dir,
            overrides: std::collections::BTreeMap::new(),
            tint: (0, 0, 0),
            tintable: false,
            generation: 0,
            external_revision: 0,
            active_pending: HashSet::new(),
            active_failed: HashSet::new(),
            picker_pending: HashSet::new(),
            picker_failed: HashSet::new(),
            icon_tx: worker_started.then_some(tx),
            icon_rx: result_rx,
            waker: None,
        }
    }

    /// Registra el aviso thread-safe que despierta Slint al terminar una lectura externa.
    pub(crate) fn set_waker(&mut self, waker: naygo_platform::dir_watch::Waker) {
        self.waker = Some(waker);
    }

    /// Reemplaza los overrides de ícono (mapa nombre → fuente). Se llama al cargar la config
    /// y cada vez que el usuario cambia un override. Invalida el cache completo: las entradas
    /// anteriores podrían haberse resuelto sin el override nuevo.
    pub fn set_overrides(
        &mut self,
        ov: std::collections::BTreeMap<String, naygo_core::icon_source::IconSource>,
    ) {
        self.overrides = ov;
        self.map.clear();
        self.control_map.clear();
        self.advance_generation();
    }

    /// Configura el tinte del set activo. `tintable` indica si el set es máscara blanca;
    /// `rgb` es el color del tema. Si `tintable` es `false`, no se aplica ningún tinte.
    /// Invalida el cache porque el color de tinte está embebido en los píxeles cacheados.
    pub fn set_tint(&mut self, tintable: bool, rgb: (u8, u8, u8)) {
        self.tintable = tintable;
        self.tint = rgb;
        self.map.clear();
        self.control_map.clear();
        self.advance_generation();
    }

    /// Solo para tests: número de entradas actualmente en el cache.
    #[cfg(test)]
    pub fn cache_len(&self) -> usize {
        self.map.len()
    }

    /// Cambia el set activo (al cambiarlo en Configuración). No borra el cache: las claves del
    /// set nuevo se decodifican on-demand; las viejas quedan (memoria despreciable, 28 PNGs).
    pub fn set_active(&mut self, set_id: impl Into<String>) {
        let new = set_id.into();
        if new != self.active {
            // El cache es solo del set vigente (la clave ya no lleva el set): al cambiar, vaciarlo.
            self.map.clear();
            self.control_map.clear();
            self.advance_generation();
            self.active = new;
        }
    }

    /// Activa o desactiva la consulta de asociaciones de Windows. El worker se crea solo al
    /// habilitar la opción; desactivarla descarta resultados pendientes y conserva el fallback.
    pub fn set_system_file_icons(&mut self, enabled: bool) {
        if self.system_file_icons == enabled {
            return;
        }
        self.system_file_icons = enabled;
        self.system_file_map.clear();
        self.system_file_requested.clear();
        self.system_file_refresh_pending = false;
        self.system_file_revision = self.system_file_revision.wrapping_add(1);
        if enabled {
            let (tx, requests) = std::sync::mpsc::sync_channel::<(
                String,
                Option<naygo_platform::dir_watch::Waker>,
            )>(256);
            let (results_tx, results_rx) = std::sync::mpsc::sync_channel::<(
                String,
                Option<naygo_platform::system_file_icons::SystemFileIcon>,
            )>(256);
            let started = std::thread::Builder::new()
                .name("naygo-shell-file-icons".into())
                .spawn(move || {
                    while let Ok((extension, wake)) = requests.recv() {
                        let icon = naygo_platform::system_file_icons::for_extension(&extension);
                        if results_tx.send((extension, icon)).is_err() {
                            break;
                        }
                        if let Some(wake) = wake {
                            wake();
                        }
                    }
                })
                .is_ok();
            if started {
                self.system_file_tx = Some(tx);
                self.system_file_rx = Some(results_rx);
            }
        } else {
            self.system_file_tx = None;
            self.system_file_rx = None;
        }
    }

    /// Firma O(k) (k = nº de overrides, normalmente 0) del estado que determina el `slint::Image`
    /// de CADA fila: set activo + tinte + tintable + overrides. Si esta firma no cambia, todos los
    /// íconos que `get` devolvería son idénticos (mismo buffer cacheado). La usa `rows_signature`
    /// para incluir el ícono en la firma por panel sin re-decodificar nada. NO aloca: hashea por
    /// referencia los bytes del id/overrides.
    pub fn signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.active.hash(&mut h);
        self.external_revision.hash(&mut h);
        self.system_file_icons.hash(&mut h);
        self.system_file_revision.hash(&mut h);
        self.tint.hash(&mut h);
        self.tintable.hash(&mut h);
        // El BTreeMap recorre ordenado → hash estable. Normalmente vacío (sin overrides).
        for (name, src) in &self.overrides {
            name.hash(&mut h);
            // `IconSource` no deriva Hash en core: hashear sus variantes a mano (un id de set o
            // una ruta relativa). El discriminante separa Builtin de UserPng con el mismo string.
            match src {
                naygo_core::icon_source::IconSource::Builtin { set_id } => {
                    0u8.hash(&mut h);
                    set_id.hash(&mut h);
                }
                naygo_core::icon_source::IconSource::UserPng { rel_path } => {
                    1u8.hash(&mut h);
                    rel_path.hash(&mut h);
                }
            }
        }
        h.finish()
    }

    /// El `slint::Image` del ícono `key` en el set activo, aplicando overrides y tinte.
    /// Lo decodifica si falta y lo cachea. Si el PNG es ilegible, devuelve imagen vacía.
    pub fn get(&mut self, key: IconKey) -> Image {
        let tint = if self.tintable { self.tint } else { (0, 0, 0) };
        let ck = (key, tint, self.tintable);
        if let Some(img) = self.map.get(&ck) {
            return img.clone();
        }
        if self.active_needs_disk(key) {
            self.request_active(key);
            let image = decode(&self.fallback_active_bytes(key));
            self.map.insert(ck, image.clone());
            return image;
        }
        let bytes = self.embedded_active_bytes(key);
        let bytes = if self.tintable {
            tint_png(&bytes, self.tint)
        } else {
            bytes
        };
        let img = decode(&bytes);
        self.map.insert(ck, img.clone());
        img
    }

    /// Controles nuevos: resolver solo assets embebidos, nunca leer PNGs de disco en el
    /// refresco de UI. Respeta los sets compilados y overrides Builtin; los packs externos
    /// conservan el fallback embebido. Caché separada para no ocultar overrides de filas.
    pub fn get_control(&mut self, key: IconKey) -> Image {
        let tint = if self.tintable { self.tint } else { (0, 0, 0) };
        let ck = (key, tint, self.tintable);
        if let Some(image) = self.control_map.get(&ck) {
            return image.clone();
        }
        let set = match self.overrides.get(naygo_core::icons::file_name(key)) {
            Some(naygo_core::icon_source::IconSource::Builtin { set_id }) => set_id,
            _ => &self.active,
        };
        let mut bytes = naygo_core::icons::bytes_for_id(set, key);
        if bytes.is_empty() {
            bytes = naygo_core::icons::bytes_for_id("lucide", key);
        }
        let bytes = if self.tintable {
            tint_png(&bytes, self.tint)
        } else {
            bytes
        };
        let image = decode(&bytes);
        self.control_map.insert(ck, image.clone());
        image
    }

    /// Ícono para un `Entry` de archivo/carpeta: si su extensión tiene ícono EXACTO por
    /// tipo (capa global vscode-icons: pdf, docx, xlsx, mp3, zip…), usa ese (sin tintar,
    /// trae su color); si no, el ícono de categoría del set activo (comportamiento de
    /// siempre). Los exactos se cachean aparte por extensión (no dependen del set/tinte).
    pub fn get_for_entry(&mut self, e: &naygo_core::fs_model::Entry) -> Image {
        if e.kind == naygo_core::fs_model::EntryKind::File {
            if let Some(ext) = e.path.extension().and_then(|x| x.to_str()) {
                let system_key = ext.to_ascii_lowercase();
                if self.system_file_icons {
                    if let Some(image) = self.system_file_map.get(&system_key) {
                        if let Some(image) = image {
                            return image.clone();
                        }
                    } else if self.system_file_requested.len() + self.system_file_map.len() < 512
                        && self.system_file_requested.insert(system_key.clone())
                    {
                        if let Some(tx) = &self.system_file_tx {
                            if tx
                                .try_send((system_key.clone(), self.waker.clone()))
                                .is_err()
                            {
                                self.system_file_requested.remove(&system_key);
                            }
                        }
                    }
                }
                if let Some(bytes) = naygo_core::icons::filetype_bytes(ext) {
                    let key = ext.to_ascii_lowercase();
                    if let Some(img) = self.ft_map.get(&key) {
                        return img.clone();
                    }
                    let img = decode(bytes);
                    self.ft_map.insert(key, img.clone());
                    return img;
                }
            }
        }
        self.get(naygo_core::icon_kind::icon_key_for(e))
    }

    /// Ícono de un set no activo, usado en galerías y selección de overrides. Los sets sueltos
    /// se leen/decodifican en el worker y muestran Lucide hasta que la muestra esté lista.
    pub(crate) fn get_for_set(
        &mut self,
        key: IconKey,
        set_id: &str,
        tintable: bool,
        tint: (u8, u8, u8),
    ) -> Image {
        let effective_tint = if tintable { tint } else { (0, 0, 0) };
        let cache_key = (set_id.to_string(), key, effective_tint, tintable);
        if let Some(image) = self.picker_map.get(&cache_key) {
            return image.clone();
        }
        if naygo_core::icons::bytes_for_id(set_id, key).is_empty() {
            self.request_picker(cache_key.clone(), tintable, tint);
            let fallback = naygo_core::icons::bytes_for_id("lucide", key);
            let fallback = if tintable {
                tint_png(&fallback, tint)
            } else {
                fallback
            };
            let image = decode(&fallback);
            self.picker_map.insert(cache_key, image.clone());
            return image;
        }
        let bytes = naygo_core::icons::bytes_for_id(set_id, key);
        let bytes = if tintable {
            tint_png(&bytes, tint)
        } else {
            bytes
        };
        let image = decode(&bytes);
        self.picker_map.insert(cache_key, image.clone());
        image
    }

    /// Drena sin bloquear los PNG externos listos. Los resultados de un set/tinte anterior se
    /// descartan por generación; los fallos conservan el fallback y no relanzan el mismo probe.
    pub(crate) fn pump_external(&mut self) -> IconPump {
        let mut changed = false;
        let mut system_file_refresh = false;
        let mut errors = Vec::new();
        while let Ok(result) = self.icon_rx.try_recv() {
            match result.target {
                IconLoadTarget::Active { generation, key } => {
                    self.active_pending.remove(&(generation, key));
                    if generation != self.generation {
                        continue;
                    }
                    let cache_key = (
                        key,
                        if self.tintable { self.tint } else { (0, 0, 0) },
                        self.tintable,
                    );
                    if let Some(decoded) = result.decoded {
                        self.map.insert(cache_key, to_slint_image(decoded));
                    } else {
                        self.active_failed.insert((generation, key));
                        self.map
                            .insert(cache_key, decode(&self.fallback_active_bytes(key)));
                        errors.push("PNG de ícono inválido o fuera de límites".to_string());
                    }
                    changed = true;
                }
                IconLoadTarget::Picker(cache_key) => {
                    self.picker_pending.remove(&cache_key);
                    if let Some(decoded) = result.decoded {
                        self.picker_map.insert(cache_key, to_slint_image(decoded));
                    } else {
                        self.picker_failed.insert(cache_key.clone());
                        errors.push("PNG de muestra inválido o fuera de límites".to_string());
                    }
                    changed = true;
                }
            }
        }
        if let Some(rx) = &self.system_file_rx {
            while let Ok((extension, icon)) = rx.try_recv() {
                self.system_file_requested.remove(&extension);
                let image = icon.map(|icon| {
                    to_slint_image(DecodedIcon {
                        width: icon.width,
                        height: icon.height,
                        rgba: icon.rgba,
                    })
                });
                self.system_file_map.insert(extension, image);
                self.system_file_refresh_pending = true;
            }
        }
        // Windows Shell може responder de a una o en pequeños grupos. Esperar a que el conjunto
        // encolado termine evita reconstruir el modelo de filas en cada tick; mientras tanto las
        // filas conservan el fallback y el resultado completo se aplica de una sola vez.
        if self.system_file_refresh_pending && self.system_file_requested.is_empty() {
            self.system_file_refresh_pending = false;
            self.system_file_revision = self.system_file_revision.wrapping_add(1);
            system_file_refresh = true;
        }
        if changed {
            self.external_revision = self.external_revision.wrapping_add(1);
        }
        IconPump {
            changed: changed || system_file_refresh,
            errors,
        }
    }

    fn advance_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.external_revision = self.external_revision.wrapping_add(1);
        self.active_pending.clear();
        self.active_failed.clear();
        self.picker_map.clear();
        self.picker_pending.clear();
        self.picker_failed.clear();
    }

    fn active_needs_disk(&self, key: IconKey) -> bool {
        match self.overrides.get(naygo_core::icons::file_name(key)) {
            Some(naygo_core::icon_source::IconSource::UserPng { .. }) => true,
            Some(naygo_core::icon_source::IconSource::Builtin { set_id }) => {
                naygo_core::icons::bytes_for_id(set_id, key).is_empty()
            }
            None => naygo_core::icons::bytes_for_id(&self.active, key).is_empty(),
        }
    }

    fn request_active(&mut self, key: IconKey) {
        let generation = self.generation;
        if self.active_pending.contains(&(generation, key))
            || self.active_failed.contains(&(generation, key))
        {
            return;
        }
        let set_id = match self.overrides.get(naygo_core::icons::file_name(key)) {
            Some(naygo_core::icon_source::IconSource::Builtin { set_id }) => set_id.clone(),
            _ => self.active.clone(),
        };
        let request = IconLoadRequest {
            target: IconLoadTarget::Active { generation, key },
            set_id,
            key,
            overrides: self.overrides.clone(),
            data_dir: self.data_dir.clone(),
            legacy_config_dir: self.legacy_config_dir.clone(),
            tintable: self.tintable,
            tint: self.tint,
            wake: self.waker.clone(),
        };
        if self
            .icon_tx
            .as_ref()
            .is_some_and(|tx| tx.try_send(request).is_ok())
        {
            self.active_pending.insert((generation, key));
        }
    }

    fn request_picker(&mut self, cache_key: PickerCacheKey, tintable: bool, tint: (u8, u8, u8)) {
        if self.picker_pending.contains(&cache_key) || self.picker_failed.contains(&cache_key) {
            return;
        }
        let (set_id, key, _, _) = &cache_key;
        let request = IconLoadRequest {
            target: IconLoadTarget::Picker(cache_key.clone()),
            set_id: set_id.clone(),
            key: *key,
            overrides: std::collections::BTreeMap::new(),
            data_dir: self.data_dir.clone(),
            legacy_config_dir: self.legacy_config_dir.clone(),
            tintable,
            tint,
            wake: self.waker.clone(),
        };
        if self
            .icon_tx
            .as_ref()
            .is_some_and(|tx| tx.try_send(request).is_ok())
        {
            self.picker_pending.insert(cache_key);
        }
    }

    fn embedded_active_bytes(&self, key: IconKey) -> Vec<u8> {
        if let Some(naygo_core::icon_source::IconSource::Builtin { set_id }) =
            self.overrides.get(naygo_core::icons::file_name(key))
        {
            if let bytes @ [_, ..] = naygo_core::icons::bytes_for_id(set_id, key).as_slice() {
                return bytes.to_vec();
            }
        }
        let bytes = naygo_core::icons::bytes_for_id(&self.active, key);
        if bytes.is_empty() {
            naygo_core::icons::bytes_for_id("lucide", IconKey::Unknown)
        } else {
            bytes
        }
    }

    fn fallback_active_bytes(&self, key: IconKey) -> Vec<u8> {
        self.embedded_active_bytes(key)
    }
}

/// Renderiza el `key` en un set específico (no el activo), aplicando tinte si corresponde.
/// Se usa en el picker de ícono para mostrar el mismo objeto en cada set disponible.
fn to_slint_image(decoded: DecodedIcon) -> Image {
    let buf = SharedPixelBuffer::clone_from_slice(&decoded.rgba, decoded.width, decoded.height);
    Image::from_rgba8(buf)
}

fn decode_external(bytes: &[u8], tintable: bool, tint: (u8, u8, u8)) -> Option<DecodedIcon> {
    const MAX_ENCODED_BYTES: usize = 8 * 1024 * 1024;
    const MAX_DIMENSION: u32 = 1024;
    const MAX_DECODED_BYTES: u64 = 4 * 1024 * 1024;
    if bytes.is_empty() || bytes.len() > MAX_ENCODED_BYTES {
        return None;
    }
    let cursor = std::io::Cursor::new(bytes);
    let mut reader = image::ImageReader::with_format(cursor, image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    reader.limits(limits);
    let mut rgba = reader.decode().ok()?.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    if tintable {
        for pixel in rgba.pixels_mut() {
            let alpha = pixel[3];
            *pixel = image::Rgba([tint.0, tint.1, tint.2, alpha]);
        }
    }
    Some(DecodedIcon {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}

/// Decodifica bytes PNG a un `slint::Image` RGBA. Imagen vacía si falla.
pub(crate) fn decode(bytes: &[u8]) -> Image {
    match image::load_from_memory_with_format(bytes, image::ImageFormat::Png) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            let buf = SharedPixelBuffer::clone_from_slice(rgba.as_raw(), w, h);
            Image::from_rgba8(buf)
        }
        Err(_) => Image::default(),
    }
}

/// Recolorea un PNG (máscara) al color `(r,g,b)` conservando su canal alfa.
/// Para sets tintables: el glifo blanco se vuelve del color del tema.
pub fn tint_png(bytes: &[u8], rgb: (u8, u8, u8)) -> Vec<u8> {
    let decoded = match image::load_from_memory(bytes) {
        Ok(d) => d.to_rgba8(),
        Err(_) => return bytes.to_vec(),
    };
    let (w, h) = (decoded.width(), decoded.height());
    let mut out = image::RgbaImage::new(w, h);
    for (x, y, p) in decoded.enumerate_pixels() {
        out.put_pixel(x, y, image::Rgba([rgb.0, rgb.1, rgb.2, p[3]]));
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    if image::DynamicImage::ImageRgba8(out)
        .write_to(&mut buf, image::ImageFormat::Png)
        .is_err()
    {
        return bytes.to_vec();
    }
    buf.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::icon_kind::IconKey;

    fn cache() -> IconCache {
        IconCache::new("lucide", std::path::PathBuf::new())
    }

    #[test]
    fn get_cachea_la_misma_clave() {
        let mut c = cache();
        let a = c.get(IconKey::Folder);
        let b = c.get(IconKey::Folder);
        // El size es estable y no vacío (el PNG de carpeta existe).
        assert_eq!(a.size(), b.size());
        assert!(a.size().width > 0);
    }

    #[test]
    fn cache_aplica_override_y_tiñe() {
        use naygo_core::icon_source::IconSource;
        use std::collections::BTreeMap;
        let mut ov: BTreeMap<String, IconSource> = BTreeMap::new();
        ov.insert(
            "folder".into(),
            IconSource::Builtin {
                set_id: "material".into(),
            },
        );
        let mut c = IconCache::new("lucide", std::path::PathBuf::new());
        c.set_overrides(ov);
        c.set_tint(true, (200, 100, 50)); // set tintable: aplica color
        let img = c.get(IconKey::Folder);
        assert!(img.size().width > 0); // se resolvió y decodificó sin panic
    }

    #[test]
    fn set_overrides_invalida_el_cache() {
        use naygo_core::icon_source::IconSource;
        use std::collections::BTreeMap;
        let mut c = IconCache::new("lucide", std::path::PathBuf::new());
        // Cachear folder del set base (lucide).
        let _ = c.get(IconKey::Folder);
        assert_eq!(
            c.cache_len(),
            1,
            "debe haber 1 entrada cacheada tras el primer get"
        );
        // Aplicar override: folder pasa a resolverse desde "material".
        let mut ov: BTreeMap<String, IconSource> = BTreeMap::new();
        ov.insert(
            "folder".into(),
            IconSource::Builtin {
                set_id: "material".into(),
            },
        );
        c.set_overrides(ov);
        assert_eq!(
            c.cache_len(),
            0,
            "set_overrides debe invalidar el cache completo"
        );
        // Re-pedir: debe re-decodificar con el override aplicado.
        let _ = c.get(IconKey::Folder);
        assert_eq!(
            c.cache_len(),
            1,
            "se re-decodifica con el override aplicado"
        );
    }

    #[test]
    fn tint_recolorea_conservando_alfa() {
        // un PNG blanco semitransparente: tras teñir a rojo, el RGB es rojo y el alfa se conserva.
        let mut buf = image::RgbaImage::new(1, 1);
        buf.put_pixel(0, 0, image::Rgba([255, 255, 255, 128]));
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(buf)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let out = tint_png(png.get_ref(), (255, 0, 0));
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        let px = img.get_pixel(0, 0);
        assert_eq!(px[0], 255);
        assert_eq!(px[1], 0);
        assert_eq!(px[2], 0);
        assert_eq!(px[3], 128, "el alfa se conserva");
    }

    #[test]
    fn png_externo_se_carga_en_worker_y_luego_reemplaza_el_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let icon_dir = tmp.path().join("icons").join("mio");
        std::fs::create_dir_all(&icon_dir).unwrap();
        let mut cursor = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            64,
            32,
            image::Rgba([20, 40, 60, 255]),
        ))
        .write_to(&mut cursor, image::ImageFormat::Png)
        .unwrap();
        std::fs::write(icon_dir.join("folder.png"), cursor.into_inner()).unwrap();

        let mut cache = IconCache::new("mio", tmp.path().to_path_buf());
        let first = cache.get(IconKey::Folder);
        assert_ne!((first.size().width, first.size().height), (64, 32));

        let loaded = (0..500).find_map(|_| {
            std::thread::sleep(std::time::Duration::from_millis(2));
            let pump = cache.pump_external();
            if pump.changed {
                Some(cache.get(IconKey::Folder))
            } else {
                None
            }
        });
        let loaded = loaded.expect("el worker entrega el PNG externo");
        assert_eq!((loaded.size().width, loaded.size().height), (64, 32));
    }

    #[test]
    fn png_externo_fuera_de_limites_conserva_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let icon_dir = tmp.path().join("icons").join("mio");
        std::fs::create_dir_all(&icon_dir).unwrap();
        let mut cursor = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(1025, 1))
            .write_to(&mut cursor, image::ImageFormat::Png)
            .unwrap();
        std::fs::write(icon_dir.join("folder.png"), cursor.into_inner()).unwrap();

        let mut cache = IconCache::new("mio", tmp.path().to_path_buf());
        let fallback = cache.get(IconKey::Folder).size();
        for _ in 0..500 {
            std::thread::sleep(std::time::Duration::from_millis(2));
            if cache.pump_external().changed {
                break;
            }
        }
        assert_eq!(cache.get(IconKey::Folder).size(), fallback);
    }
}
