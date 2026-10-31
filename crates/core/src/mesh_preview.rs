// Naygo — parseo y rasterización por CPU de previews STL/3MF.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Produce una imagen RGBA estática sin GPU ni tipos de UI. Diseñado para ejecutarse en el
//! worker cancelable de preview.

use crate::CancellationToken;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

pub const MESH_MAX_BYTES: u64 = 200 * 1024 * 1024;
pub const MESH_MAX_TRIANGLES: usize = 2_000_000;
// El límite anterior (60.000) tomaba uno de cada N triángulos de los modelos densos. Eso es
// rápido, pero deja agujeros regulares — el objeto se ve como una nube de puntos en vez de una
// superficie. 500k cubre ampliamente los STL típicos de impresión 3D; sobre ese umbral se sigue
// degradando de forma controlada para que el preview nunca se vuelva un trabajo ilimitado. La UI
// además puede cancelar el worker mientras rasteriza.
const RENDER_MAX_TRIANGLES: usize = 500_000;

#[path = "mesh_scene_wire.rs"]
mod scene_wire;
pub use scene_wire::SCENE_MAX_BYTES;
/// Scratch del rasterizador local; no incluye la escena retenida ni el frame entregado.
pub const RENDER_WORK_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Triangle([u32; 3]);

#[derive(Clone, Debug, PartialEq)]
pub struct MeshInfo {
    pub triangle_count: usize,
    pub dimensions: [f32; 3],
    pub unit: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeshPreview {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub info: MeshInfo,
}

/// Presentation of retained geometry. `Source` uses supported embedded 3MF colors where present.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeshColor {
    #[default]
    Blue,
    White,
    Warm,
    Source,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeshSurface {
    #[default]
    Solid,
    Edges,
    Wireframe,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeshAppearance {
    pub color: MeshColor,
    pub surface: MeshSurface,
}

/// Geometría ya leída de un STL/3MF. Mantenerla separada de la rasterización permite que la UI
/// rote o acerque el modelo sin volver a tocar el disco por cada movimiento del mouse.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshScene {
    mesh: Mesh,
    pub info: MeshInfo,
}

impl MeshScene {
    /// Capacidades realmente retenidas (no sólo longitudes). No estima temporales de parseo.
    pub fn retained_bytes(&self) -> Option<usize> {
        self.mesh
            .vertices
            .capacity()
            .checked_mul(std::mem::size_of::<Vec3>())?
            .checked_add(
                self.mesh
                    .triangles
                    .capacity()
                    .checked_mul(std::mem::size_of::<Triangle>())?,
            )?
            .checked_add(
                self.mesh
                    .colors
                    .capacity()
                    .checked_mul(std::mem::size_of::<Option<[u8; 4]>>())?,
            )?
            .checked_add(self.mesh.unit.capacity())?
            .checked_add(self.info.unit.capacity())
    }
}

/// Cámara ortográfica del preview 3D. La orientación es un cuaternión para que cada arrastre se
/// aplique en los ejes visibles de la cámara incluso después de inclinar o rotar el modelo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshCamera {
    orientation: [f32; 4],
    pub zoom: f32,
}

impl Default for MeshCamera {
    fn default() -> Self {
        Self {
            orientation: [1.0, 0.0, 0.0, 0.0],
            zoom: 1.0,
        }
    }
}

impl MeshCamera {
    /// Orbita alrededor de los ejes de pantalla, en grados. El delta horizontal gira sobre el
    /// eje vertical de la cámara y el vertical sobre su eje derecho. Ignora entradas no finitas.
    pub fn orbit(&mut self, horizontal_degrees: f32, vertical_degrees: f32) {
        if !horizontal_degrees.is_finite() || !vertical_degrees.is_finite() {
            return;
        }
        let half_horizontal = -horizontal_degrees.to_radians() * 0.5;
        let half_vertical = vertical_degrees.to_radians() * 0.5;
        let horizontal = [half_horizontal.cos(), 0.0, 0.0, half_horizontal.sin()];
        let vertical = [half_vertical.cos(), half_vertical.sin(), 0.0, 0.0];
        // La orientación se compone después de la proyección fija del modelo. Sus ejes X/Y son
        // literalmente los ejes horizontal/vertical de pantalla, así el arrastre sigue el eje
        // visible aunque la proyección inicial sea isométrica y el modelo ya esté inclinado.
        let delta_view = quaternion_multiply(vertical, horizontal);
        self.orientation = quaternion_normalize(quaternion_multiply(delta_view, self.orientation));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeshError {
    Unsupported,
    Unreadable,
    Invalid,
    TooLarge,
    TooManyTriangles,
    Empty,
    Cancelled,
    /// El supervisor terminó un lector por tiempo, memoria o respuesta inválida.
    ReaderStopped,
}

#[derive(Clone, Debug, PartialEq)]
struct Mesh {
    vertices: Vec<Vec3>,
    triangles: Vec<Triangle>,
    // Empty for uncolored models; otherwise exactly one optional RGBA value per triangle.
    colors: Vec<Option<[u8; 4]>>,
    unit: String,
}

/// Matriz de cámara compartida por todo el frame; la conversión de orientación cuesta una vez por
/// raster y se reutiliza para vértices y normales.
#[derive(Clone, Copy, Debug)]
struct Rotation {
    matrix: [[f32; 3]; 3],
}

impl Rotation {
    fn new(camera: MeshCamera) -> Self {
        let [w, x, y, z] = quaternion_normalize(camera.orientation);
        Self {
            matrix: [
                [
                    1.0 - 2.0 * (y * y + z * z),
                    2.0 * (x * y - w * z),
                    2.0 * (x * z + w * y),
                ],
                [
                    2.0 * (x * y + w * z),
                    1.0 - 2.0 * (x * x + z * z),
                    2.0 * (y * z - w * x),
                ],
                [
                    2.0 * (x * z - w * y),
                    2.0 * (y * z + w * x),
                    1.0 - 2.0 * (x * x + y * y),
                ],
            ],
        }
    }

    fn apply(self, point: Vec3) -> Vec3 {
        Vec3 {
            x: self.matrix[0][0] * point.x
                + self.matrix[0][1] * point.y
                + self.matrix[0][2] * point.z,
            y: self.matrix[1][0] * point.x
                + self.matrix[1][1] * point.y
                + self.matrix[1][2] * point.z,
            z: self.matrix[2][0] * point.x
                + self.matrix[2][1] * point.y
                + self.matrix[2][2] * point.z,
        }
    }
}

fn quaternion_multiply(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [aw, ax, ay, az] = a;
    let [bw, bx, by, bz] = b;
    [
        aw * bw - ax * bx - ay * by - az * bz,
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
    ]
}

fn quaternion_normalize(q: [f32; 4]) -> [f32; 4] {
    let length = q.iter().map(|value| value * value).sum::<f32>().sqrt();
    if length.is_finite() && length > f32::EPSILON {
        q.map(|value| value / length)
    } else {
        [1.0, 0.0, 0.0, 0.0]
    }
}

#[derive(Clone, Copy, Debug)]
struct Transform([[f32; 4]; 4]);

impl Transform {
    const IDENTITY: Self = Self([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    fn apply(self, point: Vec3) -> Vec3 {
        Vec3 {
            x: self.0[0][0] * point.x
                + self.0[0][1] * point.y
                + self.0[0][2] * point.z
                + self.0[0][3],
            y: self.0[1][0] * point.x
                + self.0[1][1] * point.y
                + self.0[1][2] * point.z
                + self.0[1][3],
            z: self.0[2][0] * point.x
                + self.0[2][1] * point.y
                + self.0[2][2] * point.z
                + self.0[2][3],
        }
    }

    fn then(self, child: Self) -> Self {
        let mut out = [[0.0; 4]; 4];
        for (row, values) in out.iter_mut().enumerate() {
            for (column, value) in values.iter_mut().enumerate() {
                *value = (0..4)
                    .map(|inner| self.0[row][inner] * child.0[inner][column])
                    .sum();
            }
        }
        Self(out)
    }
}

#[derive(Clone, Debug, Default)]
struct Object3mf {
    vertices: Vec<Vec3>,
    triangles: Vec<Triangle>,
    properties: Vec<Option<(u32, u32)>>,
    default_property: Option<(u32, u32)>,
    components: Vec<(u32, Transform)>,
}

type FlattenedMesh = (Vec<Vec3>, Vec<Triangle>, Vec<Option<[u8; 4]>>);

pub fn render_file(
    path: &Path,
    width: u32,
    height: u32,
    token: &CancellationToken,
) -> Result<MeshPreview, MeshError> {
    let metadata = std::fs::metadata(path).map_err(|_| MeshError::Unreadable)?;
    if metadata.len() > MESH_MAX_BYTES {
        return Err(MeshError::TooLarge);
    }
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mesh = match ext.as_str() {
        "stl" => parse_stl(path, token)?,
        "3mf" => {
            // Priorizar geometría para que la vista inicial corresponda al modelo manipulable.
            // El parser conserva sus límites; si no soporta el modelo, la miniatura permite
            // visualizar paquetes de producción. Nunca convertir una cancelación en éxito.
            match parse_3mf(path, token) {
                Ok(mesh) => mesh,
                Err(MeshError::Cancelled) => return Err(MeshError::Cancelled),
                Err(error) => match render_3mf_thumbnail(path, width, height, token) {
                    Ok(Some(preview)) => return Ok(preview),
                    Err(MeshError::Cancelled) => return Err(MeshError::Cancelled),
                    _ => return Err(error),
                },
            }
        }
        _ => return Err(MeshError::Unsupported),
    };
    rasterize(
        &mesh,
        width.clamp(64, 1024),
        height.clamp(64, 1024),
        MeshCamera::default(),
        MeshAppearance::default(),
        token,
    )
}

/// Carga la geometría real de un STL/3MF sin recurrir a su miniatura embebida: al interactuar
/// el usuario necesita los triángulos reales para
/// poder rotar el objeto. Debe ejecutarse en un worker cancelable.
pub fn load_scene(path: &Path, token: &CancellationToken) -> Result<MeshScene, MeshError> {
    let metadata = std::fs::metadata(path).map_err(|_| MeshError::Unreadable)?;
    if metadata.len() > MESH_MAX_BYTES {
        return Err(MeshError::TooLarge);
    }
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mesh = match ext.as_str() {
        "stl" => parse_stl(path, token)?,
        "3mf" => parse_3mf(path, token)?,
        _ => return Err(MeshError::Unsupported),
    };
    let (min, max) = bounds(&mesh.vertices);
    let info = MeshInfo {
        triangle_count: mesh.triangles.len(),
        dimensions: [max.x - min.x, max.y - min.y, max.z - min.z],
        unit: mesh.unit.clone(),
    };
    Ok(MeshScene { mesh, info })
}

/// Rasteriza una escena ya cargada con la cámara indicada. No realiza I/O.
pub fn render_scene(
    scene: &MeshScene,
    width: u32,
    height: u32,
    camera: MeshCamera,
    token: &CancellationToken,
) -> Result<MeshPreview, MeshError> {
    render_scene_with_appearance(
        scene,
        width,
        height,
        camera,
        MeshAppearance::default(),
        token,
    )
}

/// Renders a retained scene with a presentation choice without reading or changing the source.
pub fn render_scene_with_appearance(
    scene: &MeshScene,
    width: u32,
    height: u32,
    camera: MeshCamera,
    appearance: MeshAppearance,
    token: &CancellationToken,
) -> Result<MeshPreview, MeshError> {
    rasterize(
        &scene.mesh,
        width.clamp(64, 1024),
        height.clamp(64, 1024),
        camera,
        appearance,
        token,
    )
}

fn render_3mf_thumbnail(
    path: &Path,
    width: u32,
    height: u32,
    token: &CancellationToken,
) -> Result<Option<MeshPreview>, MeshError> {
    let file = File::open(path).map_err(|_| MeshError::Unreadable)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| MeshError::Invalid)?;
    let mut best: Option<(u8, usize)> = None;
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(|_| MeshError::Invalid)?;
        let name = entry.name().replace('\\', "/").to_ascii_lowercase();
        let score = if name == "auxiliaries/.thumbnails/thumbnail_3mf.png" {
            Some(0)
        } else if name == "metadata/plate_1.png" {
            Some(1)
        } else if name.contains("thumbnail")
            && matches!(
                Path::new(&name).extension().and_then(|ext| ext.to_str()),
                Some("png" | "jpg" | "jpeg" | "webp")
            )
        {
            Some(2)
        } else {
            None
        };
        if let Some(score) = score {
            if best.map(|(old, _)| score < old).unwrap_or(true) {
                best = Some((score, index));
            }
        }
    }
    let Some((_, index)) = best else {
        return Ok(None);
    };
    if token.is_cancelled() {
        return Err(MeshError::Cancelled);
    }
    let mut entry = zip.by_index(index).map_err(|_| MeshError::Invalid)?;
    const THUMBNAIL_MAX_BYTES: u64 = 24 * 1024 * 1024;
    if entry.size() > THUMBNAIL_MAX_BYTES {
        return Ok(None);
    }
    let mut encoded = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut encoded)
        .map_err(|_| MeshError::Invalid)?;
    if token.is_cancelled() {
        return Err(MeshError::Cancelled);
    }
    let decoded = image::load_from_memory(&encoded).map_err(|_| MeshError::Invalid)?;
    let resized = decoded.thumbnail(width.clamp(64, 1024), height.clamp(64, 1024));
    let rgba = resized.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(Some(MeshPreview {
        rgba: rgba.into_raw(),
        width,
        height,
        info: MeshInfo {
            triangle_count: 0,
            dimensions: [0.0; 3],
            unit: "millimeter".to_string(),
        },
    }))
}

/// Lee solo geometría/metadatos, sin rasterizar. Lo usa el Inspector en su worker.
pub fn inspect_file(path: &Path, token: &CancellationToken) -> Result<MeshInfo, MeshError> {
    let metadata = std::fs::metadata(path).map_err(|_| MeshError::Unreadable)?;
    if metadata.len() > MESH_MAX_BYTES {
        return Err(MeshError::TooLarge);
    }
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mesh = match ext.as_str() {
        "stl" => parse_stl(path, token)?,
        "3mf" => parse_3mf(path, token)?,
        _ => return Err(MeshError::Unsupported),
    };
    let (min, max) = bounds(&mesh.vertices);
    Ok(MeshInfo {
        triangle_count: mesh.triangles.len(),
        dimensions: [max.x - min.x, max.y - min.y, max.z - min.z],
        unit: mesh.unit,
    })
}

fn parse_stl(path: &Path, token: &CancellationToken) -> Result<Mesh, MeshError> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| MeshError::Unreadable)?
        .read_to_end(&mut bytes)
        .map_err(|_| MeshError::Unreadable)?;
    if token.is_cancelled() {
        return Err(MeshError::Cancelled);
    }
    if bytes.len() >= 84 {
        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        let expected = 84usize.saturating_add(count.saturating_mul(50));
        if count > 0 && expected <= bytes.len() {
            return parse_binary_stl(&bytes, count, token);
        }
    }
    parse_ascii_stl(&bytes, token)
}

fn parse_binary_stl(
    bytes: &[u8],
    count: usize,
    token: &CancellationToken,
) -> Result<Mesh, MeshError> {
    if count > MESH_MAX_TRIANGLES {
        return Err(MeshError::TooManyTriangles);
    }
    let mut vertices = Vec::with_capacity(count.saturating_mul(3));
    let mut triangles = Vec::with_capacity(count);
    for index in 0..count {
        if index % 2048 == 0 && token.is_cancelled() {
            return Err(MeshError::Cancelled);
        }
        let start = 84 + index * 50 + 12;
        let mut ids = [0_u32; 3];
        for (vertex, id) in ids.iter_mut().enumerate() {
            let offset = start + vertex * 12;
            let point = Vec3 {
                x: f32_at(bytes, offset)?,
                y: f32_at(bytes, offset + 4)?,
                z: f32_at(bytes, offset + 8)?,
            };
            *id = vertices.len() as u32;
            vertices.push(point);
        }
        triangles.push(Triangle(ids));
    }
    mesh_or_empty(vertices, triangles, "sin unidad")
}

fn parse_ascii_stl(bytes: &[u8], token: &CancellationToken) -> Result<Mesh, MeshError> {
    let text = std::str::from_utf8(bytes).map_err(|_| MeshError::Invalid)?;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    let mut current = Vec::with_capacity(3);
    for (index, line) in text.lines().enumerate() {
        if index % 4096 == 0 && token.is_cancelled() {
            return Err(MeshError::Cancelled);
        }
        let line = line.trim();
        let Some(rest) = line.strip_prefix("vertex") else {
            continue;
        };
        let values: Vec<f32> = rest
            .split_whitespace()
            .filter_map(|value| value.parse::<f32>().ok())
            .collect();
        if values.len() != 3 || !values.iter().all(|value| value.is_finite()) {
            return Err(MeshError::Invalid);
        }
        current.push(Vec3 {
            x: values[0],
            y: values[1],
            z: values[2],
        });
        if current.len() == 3 {
            if triangles.len() >= MESH_MAX_TRIANGLES {
                return Err(MeshError::TooManyTriangles);
            }
            let base = vertices.len() as u32;
            vertices.append(&mut current);
            triangles.push(Triangle([base, base + 1, base + 2]));
        }
    }
    mesh_or_empty(vertices, triangles, "sin unidad")
}

fn parse_3mf(path: &Path, token: &CancellationToken) -> Result<Mesh, MeshError> {
    let file = File::open(path).map_err(|_| MeshError::Unreadable)?;
    parse_3mf_reader(file, token)
}

fn parse_3mf_reader<R: Read + Seek>(
    reader: R,
    token: &CancellationToken,
) -> Result<Mesh, MeshError> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|_| MeshError::Invalid)?;
    let index = (0..zip.len())
        .find(|index| {
            zip.by_index(*index)
                .map(|file| file.name().eq_ignore_ascii_case("3D/3dmodel.model"))
                .unwrap_or(false)
        })
        .ok_or(MeshError::Invalid)?;
    let mut model = zip.by_index(index).map_err(|_| MeshError::Invalid)?;
    if model.size() > 64 * 1024 * 1024 {
        return Err(MeshError::TooLarge);
    }
    let mut xml = Vec::with_capacity(model.size() as usize);
    model
        .read_to_end(&mut xml)
        .map_err(|_| MeshError::Invalid)?;
    if token.is_cancelled() {
        return Err(MeshError::Cancelled);
    }
    parse_3mf_xml(&xml, token)
}

fn parse_3mf_xml(xml: &[u8], token: &CancellationToken) -> Result<Mesh, MeshError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut objects: HashMap<u32, Object3mf> = HashMap::new();
    let mut materials: HashMap<u32, Vec<Option<[u8; 4]>>> = HashMap::new();
    let mut current_materials = None;
    let mut build = Vec::new();
    let mut current_object = None;
    let mut unit = "millimeter".to_string();
    let mut events = 0usize;
    loop {
        if events.is_multiple_of(4096) && token.is_cancelled() {
            return Err(MeshError::Cancelled);
        }
        events += 1;
        match reader.read_event() {
            Ok(Event::Start(event)) => match event.local_name().as_ref() {
                b"model" => {
                    if let Some(value) = attr(&event, b"unit") {
                        unit = value;
                    }
                }
                b"object" => {
                    let id = attr_u32(&event, b"id")?;
                    let default_property =
                        optional_u32(&event, b"pid").zip(optional_u32(&event, b"pindex"));
                    if objects
                        .insert(
                            id,
                            Object3mf {
                                default_property,
                                ..Default::default()
                            },
                        )
                        .is_some()
                    {
                        return Err(MeshError::Invalid);
                    }
                    current_object = Some(id);
                }
                b"basematerials" => {
                    let id = attr_u32(&event, b"id")?;
                    if materials.insert(id, Vec::new()).is_some() {
                        return Err(MeshError::Invalid);
                    }
                    current_materials = Some(id);
                }
                b"base" if current_materials.is_some() => {
                    let group = materials
                        .get_mut(&current_materials.unwrap())
                        .ok_or(MeshError::Invalid)?;
                    group.push(
                        attr_plain(&event, b"displaycolor")
                            .and_then(|value| parse_3mf_color(&value)),
                    );
                }
                _ => {}
            },
            Ok(Event::Empty(event)) => match event.local_name().as_ref() {
                b"base" if current_materials.is_some() => {
                    let group = materials
                        .get_mut(&current_materials.unwrap())
                        .ok_or(MeshError::Invalid)?;
                    group.push(
                        attr_plain(&event, b"displaycolor")
                            .and_then(|value| parse_3mf_color(&value)),
                    );
                }
                b"vertex" => {
                    let object = current_object
                        .and_then(|id| objects.get_mut(&id))
                        .ok_or(MeshError::Invalid)?;
                    let x = attr_f32(&event, b"x")?;
                    let y = attr_f32(&event, b"y")?;
                    let z = attr_f32(&event, b"z")?;
                    object.vertices.push(Vec3 { x, y, z });
                }
                b"triangle" => {
                    let object = current_object
                        .and_then(|id| objects.get_mut(&id))
                        .ok_or(MeshError::Invalid)?;
                    if object.triangles.len() >= MESH_MAX_TRIANGLES {
                        return Err(MeshError::TooManyTriangles);
                    }
                    object.triangles.push(Triangle([
                        attr_u32(&event, b"v1")?,
                        attr_u32(&event, b"v2")?,
                        attr_u32(&event, b"v3")?,
                    ]));
                    let property = optional_u32(&event, b"pid")
                        .or(object.default_property.map(|p| p.0))
                        .zip(optional_u32(&event, b"p1"))
                        .or(object.default_property);
                    if property.is_some() || !object.properties.is_empty() {
                        if object.properties.is_empty() {
                            object.properties.resize(object.triangles.len() - 1, None);
                        }
                        object.properties.push(property);
                    }
                }
                b"component" => {
                    // Production Extension puede referir otro archivo con el mismo objectid;
                    // resolverlo como local produciría geometría incorrecta o incompleta.
                    if attr(&event, b"path").is_some() {
                        return Err(MeshError::Unsupported);
                    }
                    let object = current_object
                        .and_then(|id| objects.get_mut(&id))
                        .ok_or(MeshError::Invalid)?;
                    object
                        .components
                        .push((attr_u32(&event, b"objectid")?, transform_attr(&event)?));
                }
                b"item" => {
                    if attr(&event, b"path").is_some() {
                        return Err(MeshError::Unsupported);
                    }
                    build.push((attr_u32(&event, b"objectid")?, transform_attr(&event)?));
                }
                _ => {}
            },
            Ok(Event::End(event)) if event.local_name().as_ref() == b"object" => {
                current_object = None;
            }
            Ok(Event::End(event)) if event.local_name().as_ref() == b"basematerials" => {
                current_materials = None;
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(MeshError::Invalid),
            _ => {}
        }
    }

    for object in objects.values() {
        if object.triangles.iter().any(|triangle| {
            triangle
                .0
                .iter()
                .any(|index| *index as usize >= object.vertices.len())
        }) {
            return Err(MeshError::Invalid);
        }
    }
    let roots: Vec<(u32, Transform)> = if build.is_empty() {
        let referenced: HashSet<u32> = objects
            .values()
            .flat_map(|object| object.components.iter().map(|(id, _)| *id))
            .collect();
        objects
            .keys()
            .filter(|id| !referenced.contains(id))
            .map(|id| (*id, Transform::IDENTITY))
            .collect()
    } else {
        build
    };
    let mut output = (Vec::new(), Vec::new(), Vec::new());
    let mut visiting = HashSet::new();
    for (id, transform) in roots {
        append_3mf_object(
            id,
            transform,
            &objects,
            &materials,
            &mut visiting,
            &mut output,
            token,
        )?;
    }
    let (vertices, triangles, mut colors) = output;
    if colors.iter().all(Option::is_none) {
        colors.clear();
        colors.shrink_to_fit();
    }
    let mut mesh = mesh_or_empty(vertices, triangles, &unit)?;
    mesh.colors = colors;
    Ok(mesh)
}

fn append_3mf_object(
    id: u32,
    transform: Transform,
    objects: &HashMap<u32, Object3mf>,
    materials: &HashMap<u32, Vec<Option<[u8; 4]>>>,
    visiting: &mut HashSet<u32>,
    output: &mut FlattenedMesh,
    token: &CancellationToken,
) -> Result<(), MeshError> {
    if token.is_cancelled() {
        return Err(MeshError::Cancelled);
    }
    if !visiting.insert(id) {
        return Err(MeshError::Invalid);
    }
    let object = objects.get(&id).ok_or(MeshError::Invalid)?;
    if output.1.len().saturating_add(object.triangles.len()) > MESH_MAX_TRIANGLES {
        visiting.remove(&id);
        return Err(MeshError::TooManyTriangles);
    }
    let base = u32::try_from(output.0.len()).map_err(|_| MeshError::TooManyTriangles)?;
    output.0.extend(
        object
            .vertices
            .iter()
            .copied()
            .map(|point| transform.apply(point)),
    );
    output.1.extend(object.triangles.iter().map(|triangle| {
        Triangle([
            base + triangle.0[0],
            base + triangle.0[1],
            base + triangle.0[2],
        ])
    }));
    if !materials.is_empty() {
        if object.properties.is_empty() {
            output
                .2
                .extend(std::iter::repeat_n(None, object.triangles.len()));
        } else {
            for (ordinal, property) in object.properties.iter().enumerate() {
                if ordinal.is_multiple_of(4096) && token.is_cancelled() {
                    return Err(MeshError::Cancelled);
                }
                output.2.push(property.and_then(|(id, index)| {
                    materials.get(&id)?.get(index as usize).copied().flatten()
                }));
            }
        }
    }
    for (child_id, child_transform) in &object.components {
        append_3mf_object(
            *child_id,
            transform.then(*child_transform),
            objects,
            materials,
            visiting,
            output,
            token,
        )?;
    }
    visiting.remove(&id);
    Ok(())
}

fn transform_attr(event: &quick_xml::events::BytesStart<'_>) -> Result<Transform, MeshError> {
    let Some(raw) = attr(event, b"transform") else {
        return Ok(Transform::IDENTITY);
    };
    let values = raw
        .split_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| MeshError::Invalid)?;
    if values.len() != 12 || values.iter().any(|value| !value.is_finite()) {
        return Err(MeshError::Invalid);
    }
    // 3MF serializa una matriz afín de 3x4 por filas de vectores; se transpone a la
    // convención columna usada internamente para que parent * child componga naturalmente.
    Ok(Transform([
        [values[0], values[3], values[6], values[9]],
        [values[1], values[4], values[7], values[10]],
        [values[2], values[5], values[8], values[11]],
        [0.0, 0.0, 0.0, 1.0],
    ]))
}

fn attr(event: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
    event
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().as_ref() == key)
        .map(|attribute| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
}

// Core material attributes are unqualified. Do not interpret vendor-prefixed attributes as them.
fn attr_plain(event: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
    event
        .attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(a.value.as_ref()).into_owned())
}

fn optional_u32(event: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<u32> {
    attr_plain(event, key).and_then(|value| value.parse().ok())
}

fn parse_3mf_color(value: &str) -> Option<[u8; 4]> {
    let bytes = value.as_bytes();
    if bytes.first() != Some(&b'#') || !matches!(bytes.len(), 7 | 9) {
        return None;
    }
    let hex = |i: usize| {
        Some(
            (bytes.get(i)?.to_ascii_lowercase() as char).to_digit(16)? as u8 * 16
                + (bytes.get(i + 1)?.to_ascii_lowercase() as char).to_digit(16)? as u8,
        )
    };
    Some([
        hex(1)?,
        hex(3)?,
        hex(5)?,
        if bytes.len() == 9 { hex(7)? } else { 255 },
    ])
}

fn attr_f32(event: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Result<f32, MeshError> {
    let value = attr(event, key)
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .ok_or(MeshError::Invalid)?;
    Ok(value)
}

fn attr_u32(event: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Result<u32, MeshError> {
    attr(event, key)
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or(MeshError::Invalid)
}

fn mesh_or_empty(
    vertices: Vec<Vec3>,
    triangles: Vec<Triangle>,
    unit: &str,
) -> Result<Mesh, MeshError> {
    if vertices.is_empty() || triangles.is_empty() {
        Err(MeshError::Empty)
    } else {
        Ok(Mesh {
            vertices,
            triangles,
            colors: Vec::new(),
            unit: unit.to_string(),
        })
    }
}

fn f32_at(bytes: &[u8], offset: usize) -> Result<f32, MeshError> {
    let slice = bytes.get(offset..offset + 4).ok_or(MeshError::Invalid)?;
    let value = f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]);
    value.is_finite().then_some(value).ok_or(MeshError::Invalid)
}

fn rasterize(
    mesh: &Mesh,
    width: u32,
    height: u32,
    camera: MeshCamera,
    appearance: MeshAppearance,
    token: &CancellationToken,
) -> Result<MeshPreview, MeshError> {
    let (min, max) = bounds(&mesh.vertices);
    if token.is_cancelled() {
        return Err(MeshError::Cancelled);
    }
    let dimensions = [max.x - min.x, max.y - min.y, max.z - min.z];
    let center = Vec3 {
        x: (min.x + max.x) * 0.5,
        y: (min.y + max.y) * 0.5,
        z: (min.z + max.z) * 0.5,
    };
    let rotation = Rotation::new(camera);
    // Admission before allocation. A retained scene cannot create unbounded scratch on
    // each camera gesture. Transform in place instead of keeping two vertex projections.
    let pixels = width as usize * height as usize;
    let scratch = mesh
        .vertices
        .len()
        .checked_mul(12)
        .and_then(|n| n.checked_add(pixels * 8))
        .ok_or(MeshError::TooLarge)?;
    if scratch > RENDER_WORK_MAX_BYTES {
        return Err(MeshError::TooLarge);
    }
    let mut projected = Vec::new();
    projected
        .try_reserve_exact(mesh.vertices.len())
        .map_err(|_| MeshError::TooLarge)?;
    for (index, point) in mesh.vertices.iter().enumerate() {
        if index % 1024 == 0 && token.is_cancelled() {
            return Err(MeshError::Cancelled);
        }
        projected.push(orient_projected(sub(*point, center), rotation));
    }
    let min_x = projected.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let max_x = projected
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = projected.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let max_y = projected
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let span_x = (max_x - min_x).max(0.0001);
    let span_y = (max_y - min_y).max(0.0001);
    let zoom = camera.zoom.clamp(0.25, 4.0);
    let scale = (((width as f32) * 0.86) / span_x).min(((height as f32) * 0.86) / span_y) * zoom;
    for (index, point) in projected.iter_mut().enumerate() {
        if index % 1024 == 0 && token.is_cancelled() {
            return Err(MeshError::Cancelled);
        }
        point[0] = (point[0] - (min_x + max_x) * 0.5) * scale + width as f32 * 0.5;
        point[1] = (point[1] - (min_y + max_y) * 0.5) * -scale + height as f32 * 0.5;
    }
    let screen = projected;
    let mut rgba = vec![0_u8; width as usize * height as usize * 4];
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[28, 31, 36, 255]);
    }
    let mut depth = vec![f32::NEG_INFINITY; width as usize * height as usize];
    // Renderizar todos los triángulos de los modelos habituales conserva una superficie sólida.
    // Solo los modelos excepcionalmente densos usan muestreo; el token se consulta aun durante
    // ese trabajo para que cancelar desde la UI responda en pocos milisegundos.
    let stride = mesh.triangles.len().div_ceil(RENDER_MAX_TRIANGLES).max(1);
    for (ordinal, triangle) in mesh.triangles.iter().step_by(stride).enumerate() {
        if ordinal % 256 == 0 && token.is_cancelled() {
            return Err(MeshError::Cancelled);
        }
        let [a, b, c] = triangle.0;
        let Some((&pa, &pb, &pc)) = screen
            .get(a as usize)
            .zip(screen.get(b as usize))
            .zip(screen.get(c as usize))
            .map(|((a, b), c)| (a, b, c))
        else {
            continue;
        };
        let normal = rotation.apply(cross(
            sub(mesh.vertices[b as usize], mesh.vertices[a as usize]),
            sub(mesh.vertices[c as usize], mesh.vertices[a as usize]),
        ));
        let length = (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z).sqrt();
        let light = if length > 0.0 {
            ((normal.x * 0.35 - normal.y * 0.25 + normal.z * 0.9) / length)
                .abs()
                .clamp(0.15, 1.0)
        } else {
            0.2
        };
        let color = match appearance.color {
            MeshColor::Source
                if mesh
                    .colors
                    .get(ordinal * stride)
                    .copied()
                    .flatten()
                    .is_some() =>
            {
                let [r, g, b, alpha] = mesh.colors[ordinal * stride].unwrap();
                // Approximate translucency against the preview background; depth remains opaque.
                let opacity = alpha as f32 / 255.0;
                let shade = 0.45 + 0.5 * light;
                let blend = |c: u8, background: u8| {
                    (c as f32 * shade * opacity + background as f32 * (1.0 - opacity)) as u8
                };
                [blend(r, 28), blend(g, 31), blend(b, 36), 255]
            }
            MeshColor::Blue | MeshColor::Source => [
                (66.0 + 66.0 * light) as u8,
                (132.0 + 78.0 * light) as u8,
                (174.0 + 64.0 * light) as u8,
                255,
            ],
            MeshColor::White => {
                let shade = (125.0 + 115.0 * light) as u8;
                [shade, shade, shade, 255]
            }
            MeshColor::Warm => [
                (136.0 + 88.0 * light) as u8,
                (101.0 + 80.0 * light) as u8,
                (62.0 + 68.0 * light) as u8,
                255,
            ],
        };
        fill_triangle(
            pa,
            pb,
            pc,
            (color, appearance.surface),
            [width, height],
            (&mut rgba, &mut depth),
            token,
        )?;
    }
    Ok(MeshPreview {
        rgba,
        width,
        height,
        info: MeshInfo {
            triangle_count: mesh.triangles.len(),
            dimensions,
            unit: mesh.unit.clone(),
        },
    })
}

fn fill_triangle(
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    presentation: ([u8; 4], MeshSurface),
    size: [u32; 2],
    buffers: (&mut [u8], &mut [f32]),
    token: &CancellationToken,
) -> Result<(), MeshError> {
    let (color, surface) = presentation;
    let (rgba, depth) = buffers;
    if token.is_cancelled() {
        return Err(MeshError::Cancelled);
    }
    let [width, height] = size;
    let area = edge(a, b, c[0], c[1]);
    if area.abs() < 0.0001 {
        return Ok(());
    }
    let min_x = a[0].min(b[0]).min(c[0]).floor().max(0.0) as u32;
    let max_x = a[0].max(b[0]).max(c[0]).ceil().min(width as f32 - 1.0) as u32;
    let min_y = a[1].min(b[1]).min(c[1]).floor().max(0.0) as u32;
    let max_y = a[1].max(b[1]).max(c[1]).ceil().min(height as f32 - 1.0) as u32;
    let inverse_area = 1.0 / area;
    // Barycentric coordinates multiplied by triangle heights give distances in pixels.
    // Computing them only for edge modes avoids extra work for the default solid preview.
    let heights = if surface == MeshSurface::Solid {
        [0.0; 3]
    } else {
        let length = |p: [f32; 3], q: [f32; 3]| {
            ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2))
                .sqrt()
                .max(0.0001)
        };
        [
            area.abs() / length(b, c),
            area.abs() / length(c, a),
            area.abs() / length(a, b),
        ]
    };
    let start_x = min_x as f32 + 0.5;
    let mut row_w0 = edge(b, c, start_x, min_y as f32 + 0.5) * inverse_area;
    let mut row_w1 = edge(c, a, start_x, min_y as f32 + 0.5) * inverse_area;
    let step_w0_x = (c[1] - b[1]) * inverse_area;
    let step_w1_x = (a[1] - c[1]) * inverse_area;
    let step_w0_y = -(c[0] - b[0]) * inverse_area;
    let step_w1_y = -(a[0] - c[0]) * inverse_area;
    for y in min_y..=max_y {
        // Un único triángulo puede cubrir todo el canvas. Consultar también durante
        // el relleno, no sólo cada 256 triángulos, acota la retirada cooperativa.
        if y % 16 == 0 && token.is_cancelled() {
            return Err(MeshError::Cancelled);
        }
        let mut w0 = row_w0;
        let mut w1 = row_w1;
        for x in min_x..=max_x {
            let w2 = 1.0 - w0 - w1;
            if w0 >= -0.0001 && w1 >= -0.0001 && w2 >= -0.0001 {
                let z = w0 * a[2] + w1 * b[2] + w2 * c[2];
                let index = y as usize * width as usize + x as usize;
                if z > depth[index] {
                    depth[index] = z;
                    let edge_pixel = surface != MeshSurface::Solid
                        && (w0 * heights[0]).min(w1 * heights[1]).min(w2 * heights[2]) <= 0.85;
                    let pixel = match (surface, edge_pixel) {
                        (MeshSurface::Wireframe, false) => [28, 31, 36, 255],
                        (MeshSurface::Wireframe, true) => color,
                        (MeshSurface::Edges, true) => [25, 28, 32, 255],
                        _ => color,
                    };
                    rgba[index * 4..index * 4 + 4].copy_from_slice(&pixel);
                }
            }
            w0 += step_w0_x;
            w1 += step_w1_x;
        }
        row_w0 += step_w0_y;
        row_w1 += step_w1_y;
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn cancelled_triangle_does_not_touch_output_buffers() {
    let token = CancellationToken::new();
    token.cancel();
    let mut rgba = vec![0; 16 * 16 * 4];
    let mut depth = vec![f32::NEG_INFINITY; 16 * 16];
    let result = fill_triangle(
        [0., 0., 0.],
        [15., 0., 0.],
        [0., 15., 0.],
        ([255; 4], MeshSurface::Solid),
        [16, 16],
        (&mut rgba, &mut depth),
        &token,
    );
    assert!(matches!(result, Err(MeshError::Cancelled)));
    assert!(rgba.iter().all(|byte| *byte == 0));
    assert!(depth.iter().all(|value| *value == f32::NEG_INFINITY));
}

#[cfg(test)]
#[test]
fn wireframe_hides_edges_behind_a_nearer_surface() {
    let token = CancellationToken::new();
    let mut rgba = vec![0; 32 * 32 * 4];
    let mut depth = vec![f32::NEG_INFINITY; 32 * 32];
    let far = [[2., 10., 0.], [29., 10., 0.], [15., 28., 0.]];
    let near = [[2., 2., 1.], [29., 2., 1.], [15., 29., 1.]];
    for points in [far, near] {
        fill_triangle(
            points[0],
            points[1],
            points[2],
            ([240; 4], MeshSurface::Wireframe),
            [32, 32],
            (&mut rgba, &mut depth),
            &token,
        )
        .unwrap();
    }
    let pixel = (10 * 32 + 15) * 4;
    assert_eq!(&rgba[pixel..pixel + 4], &[28, 31, 36, 255]);
}

fn edge(a: [f32; 3], b: [f32; 3], x: f32, y: f32) -> f32 {
    (x - a[0]) * (b[1] - a[1]) - (y - a[1]) * (b[0] - a[0])
}

fn project(point: Vec3) -> [f32; 3] {
    [
        (point.x - point.y) * 0.707_106_77,
        (point.x + point.y) * 0.408_248_3 + point.z * 0.816_496_6,
        (point.x + point.y + point.z) * 0.577_350_26,
    ]
}

fn orient_projected(point: Vec3, rotation: Rotation) -> [f32; 3] {
    let projected = project(point);
    let view = rotation.apply(Vec3 {
        x: projected[0],
        y: projected[1],
        z: projected[2],
    });
    [view.x, view.y, view.z]
}

fn bounds(vertices: &[Vec3]) -> (Vec3, Vec3) {
    let mut min = Vec3 {
        x: f32::INFINITY,
        y: f32::INFINITY,
        z: f32::INFINITY,
    };
    let mut max = Vec3 {
        x: f32::NEG_INFINITY,
        y: f32::NEG_INFINITY,
        z: f32::NEG_INFINITY,
    };
    for point in vertices {
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        min.z = min.z.min(point.z);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
        max.z = max.z.max(point.z);
    }
    (min, max)
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 {
        x: a.y * b.z - a.z * b.y,
        y: a.z * b.x - a.x * b.z,
        z: a.x * b.y - a.y * b.x,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn ascii_stl_rasteriza_y_mide() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("tri.stl");
        std::fs::write(&path, b"solid x\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 10 0 0\nvertex 0 20 0\nendloop\nendfacet\nendsolid\n").unwrap();
        let preview = render_file(&path, 128, 96, &CancellationToken::new()).unwrap();
        assert_eq!(preview.info.triangle_count, 1);
        assert_eq!(preview.info.dimensions, [10.0, 20.0, 0.0]);
        assert!(preview
            .rgba
            .chunks_exact(4)
            .any(|pixel| pixel != [28, 31, 36, 255]));
    }

    #[test]
    fn binary_stl_minimo_se_lee() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("tri.stl");
        let mut bytes = vec![0_u8; 84];
        bytes[80..84].copy_from_slice(&1_u32.to_le_bytes());
        let values = [
            0.0_f32, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        File::create(&path).unwrap().write_all(&bytes).unwrap();
        assert_eq!(
            render_file(&path, 64, 64, &CancellationToken::new())
                .unwrap()
                .info
                .triangle_count,
            1
        );
    }

    #[test]
    fn xml_3mf_base_se_parsea() {
        let xml = br#"<model unit="millimeter"><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#;
        let mesh = parse_3mf_xml(xml, &CancellationToken::new()).unwrap();
        assert_eq!(mesh.triangles.len(), 1);
        assert!(mesh.colors.is_empty());
        assert_eq!(mesh.unit, "millimeter");
    }

    #[test]
    fn materiales_base_respetan_default_por_objeto_y_override_por_triangulo() {
        let xml = br##"<model><resources>
          <basematerials id="4"><base name="red" displaycolor="#FF0000"/><base name="green" displaycolor="#00FF0080"></base></basematerials>
          <basematerials id="5"><base name="blue" displaycolor="#0000FFFF"/></basematerials>
          <object id="1" pid="4" pindex="0"><mesh><vertices>
            <vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/>
          </vertices><triangles><triangle v1="0" v2="1" v3="2"/>
            <triangle v1="0" v2="1" v3="2" p1="1"/>
            <triangle v1="0" v2="1" v3="2" pid="5" p1="0"/>
            <triangle v1="0" v2="1" v3="2" pid="99" p1="0"/>
          </triangles></mesh></object></resources><build><item objectid="1"/><item objectid="1"/></build></model>"##;
        let mesh = parse_3mf_xml(xml, &CancellationToken::new()).unwrap();
        let expected = [
            Some([255, 0, 0, 255]),
            Some([0, 255, 0, 128]),
            Some([0, 0, 255, 255]),
            None,
        ];
        assert_eq!(mesh.colors, expected.repeat(2));
        assert_eq!(mesh.colors.len(), mesh.triangles.len());
        assert_eq!(parse_3mf_color("#ff00GG"), None);
        assert_eq!(parse_3mf_color("#123"), None);
    }

    #[test]
    fn raster_con_color_de_archivo_y_respaldo_para_material_no_soportado() {
        let xml = br##"<model><resources><basematerials id="4"><base name="red" displaycolor="#FF0000"/></basematerials>
          <object id="1" pid="4" pindex="0"><mesh><vertices>
            <vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/>
          </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"##;
        let mut mesh = parse_3mf_xml(xml, &CancellationToken::new()).unwrap();
        let token = CancellationToken::new();
        let appearance = MeshAppearance {
            color: MeshColor::Source,
            surface: MeshSurface::Solid,
        };
        let source = rasterize(&mesh, 80, 80, MeshCamera::default(), appearance, &token).unwrap();
        let blue = rasterize(
            &mesh,
            80,
            80,
            MeshCamera::default(),
            MeshAppearance::default(),
            &token,
        )
        .unwrap();
        assert!(source
            .rgba
            .chunks_exact(4)
            .any(|p| p[0] > 60 && p[1] == 0 && p[2] == 0));
        assert_ne!(source.rgba, blue.rgba);
        mesh.colors[0] = Some([255, 0, 0, 128]);
        let translucent =
            rasterize(&mesh, 80, 80, MeshCamera::default(), appearance, &token).unwrap();
        assert!(translucent
            .rgba
            .chunks_exact(4)
            .any(|p| p[0] > 60 && p[1] > 0 && p[2] > 0));
        assert_ne!(translucent.rgba, source.rgba);
        mesh.colors.clear();
        let fallback = rasterize(&mesh, 80, 80, MeshCamera::default(), appearance, &token).unwrap();
        assert_eq!(fallback.rgba, blue.rgba);
    }

    #[test]
    fn build_3mf_aplica_transformacion_y_omite_objetos_no_instanciados() {
        let xml = br#"<model unit="inch"><resources>
            <object id="1"><mesh><vertices>
              <vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/>
            </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object>
            <object id="99"><mesh><vertices>
              <vertex x="0" y="0" z="0"/><vertex x="100" y="0" z="0"/><vertex x="0" y="100" z="0"/>
            </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object>
          </resources><build><item objectid="1" transform="2 0 0 0 3 0 0 0 1 10 20 30"/></build></model>"#;
        let mesh = parse_3mf_xml(xml, &CancellationToken::new()).unwrap();
        let (min, max) = bounds(&mesh.vertices);
        assert_eq!(mesh.unit, "inch");
        assert_eq!(
            mesh.triangles.len(),
            1,
            "el build controla qué se instancia"
        );
        assert_eq!(
            min,
            Vec3 {
                x: 10.0,
                y: 20.0,
                z: 30.0
            }
        );
        assert_eq!(
            max,
            Vec3 {
                x: 12.0,
                y: 23.0,
                z: 30.0
            }
        );
    }

    #[test]
    fn componentes_3mf_componen_transformaciones_anidadas() {
        let xml = br#"<model><resources>
            <object id="1"><mesh><vertices>
              <vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/>
            </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object>
            <object id="2"><components><component objectid="1" transform="1 0 0 0 1 0 0 0 1 5 0 0"/></components></object>
          </resources><build><item objectid="2" transform="1 0 0 0 1 0 0 0 1 10 0 0"/></build></model>"#;
        let mesh = parse_3mf_xml(xml, &CancellationToken::new()).unwrap();
        let (min, max) = bounds(&mesh.vertices);
        assert_eq!(mesh.triangles.len(), 1);
        assert_eq!(min.x, 15.0);
        assert_eq!(max.x, 16.0);
    }

    #[test]
    fn ciclo_de_componentes_3mf_se_rechaza_sin_recursion_infinita() {
        let xml = br#"<model><resources>
            <object id="1"><components><component objectid="2"/></components></object>
            <object id="2"><components><component objectid="1"/></components></object>
          </resources><build><item objectid="1"/></build></model>"#;
        assert_eq!(
            parse_3mf_xml(xml, &CancellationToken::new()).unwrap_err(),
            MeshError::Invalid
        );
    }

    #[test]
    fn indices_fuera_del_objeto_3mf_se_rechazan() {
        let xml = br#"<model><resources><object id="1"><mesh><vertices>
            <vertex x="0" y="0" z="0"/>
          </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles>
          </mesh></object></resources><build><item objectid="1"/></build></model>"#;
        assert_eq!(
            parse_3mf_xml(xml, &CancellationToken::new()).unwrap_err(),
            MeshError::Invalid
        );
    }

    #[test]
    fn parseo_3mf_cancelado_no_produce_malla_parcial() {
        let token = CancellationToken::new();
        token.cancel();
        assert_eq!(
            parse_3mf_xml(b"<model/>", &token).unwrap_err(),
            MeshError::Cancelled
        );
    }

    #[test]
    fn archivo_3mf_zip_ejercita_el_flujo_publico_completo() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("pieza.3mf");
        let xml = br#"<model unit="centimeter"><resources><object id="7"><mesh><vertices>
            <vertex x="0" y="0" z="0"/><vertex x="4" y="0" z="0"/><vertex x="0" y="2" z="1"/>
          </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object>
          </resources><build><item objectid="7"/></build></model>"#;
        {
            let file = File::create(&path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(xml).unwrap();
            zip.finish().unwrap();
        }
        let info = inspect_file(&path, &CancellationToken::new()).unwrap();
        assert_eq!(info.triangle_count, 1);
        assert_eq!(info.dimensions, [4.0, 2.0, 1.0]);
        assert_eq!(info.unit, "centimeter");
        let preview = render_file(&path, 96, 96, &CancellationToken::new()).unwrap();
        assert_eq!((preview.width, preview.height), (96, 96));
    }

    #[test]
    fn geometria_3mf_tiene_prioridad_sobre_miniatura() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("pieza.3mf");
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            32,
            16,
            image::Rgba([10, 120, 220, 255]),
        ))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"<model><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="4" y="0" z="0"/><vertex x="0" y="2" z="1"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources><build><item objectid="1"/></build></model>"#).unwrap();
        zip.start_file(
            "Metadata/thumbnail.png",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(png.get_ref()).unwrap();
        zip.finish().unwrap();
        let preview = render_file(&path, 160, 120, &CancellationToken::new()).unwrap();
        assert_eq!((preview.width, preview.height), (160, 120));
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert_eq!(
            render_file(&path, 160, 120, &cancelled).unwrap_err(),
            MeshError::Cancelled
        );
    }

    #[test]
    fn componente_externo_no_se_confunde_con_id_local() {
        let xml = br#"<model xmlns:p="http://schemas.microsoft.com/3dmanufacturing/production/2015/06"><resources>
        <object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object>
        <object id="2"><components><component p:path="/3D/Objects/otro.model" objectid="1"/></components></object>
        </resources><build><item objectid="2"/></build></model>"#;
        assert_eq!(
            parse_3mf_xml(xml, &CancellationToken::new()).unwrap_err(),
            MeshError::Unsupported
        );
    }

    #[test]
    fn thumbnail_incrustado_permite_preview_de_3mf_de_produccion() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("slicer.3mf");
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            320,
            200,
            image::Rgba([10, 120, 220, 255]),
        ))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
        {
            let file = File::create(&path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default())
                .unwrap();
            // Un modelo raíz con partes externas no soportadas por el parser geométrico base.
            zip.write_all(br#"<model><resources><object id="1"><components><component p:path="/3D/Objects/a.model" objectid="2"/></components></object></resources><build><item objectid="1"/></build></model>"#).unwrap();
            zip.start_file(
                "Auxiliaries/.thumbnails/thumbnail_3mf.png",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(png.get_ref()).unwrap();
            zip.finish().unwrap();
        }

        let preview = render_file(&path, 160, 120, &CancellationToken::new()).unwrap();
        assert_eq!((preview.width, preview.height), (160, 100));
        assert_eq!(preview.rgba.len(), 160 * 100 * 4);
    }

    #[test]
    fn la_camara_cambia_el_raster_sin_volver_a_leer_el_archivo() {
        let mesh = Mesh {
            vertices: vec![
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 4.0,
                    y: 0.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 1.0,
                    z: 3.0,
                },
            ],
            triangles: vec![Triangle([0, 1, 2])],
            colors: Vec::new(),
            unit: "millimeter".into(),
        };
        let scene = MeshScene {
            info: MeshInfo {
                triangle_count: 1,
                dimensions: [4.0, 1.0, 3.0],
                unit: "millimeter".into(),
            },
            mesh,
        };
        let plain = render_scene(
            &scene,
            128,
            128,
            MeshCamera::default(),
            &CancellationToken::new(),
        )
        .unwrap();
        let white = render_scene_with_appearance(
            &scene,
            128,
            128,
            MeshCamera::default(),
            MeshAppearance {
                color: MeshColor::White,
                surface: MeshSurface::Solid,
            },
            &CancellationToken::new(),
        )
        .unwrap();
        let wire = render_scene_with_appearance(
            &scene,
            128,
            128,
            MeshCamera::default(),
            MeshAppearance {
                color: MeshColor::Blue,
                surface: MeshSurface::Wireframe,
            },
            &CancellationToken::new(),
        )
        .unwrap();
        assert_ne!(plain.rgba, white.rgba);
        assert_ne!(plain.rgba, wire.rgba);
        assert_eq!(plain.info, white.info);
        assert_eq!(plain.info, wire.info);
        let mut camera = MeshCamera::default();
        camera.orbit(38.0, 16.0);
        let rotated = render_scene(&scene, 128, 128, camera, &CancellationToken::new()).unwrap();
        assert_ne!(plain.rgba, rotated.rgba);
    }

    #[test]
    fn orbit_deltas_are_composed_in_camera_axes_after_model_is_tilted() {
        let mut camera = MeshCamera::default();
        camera.orbit(38.0, 24.0);
        let point = Vec3 {
            x: 0.4,
            y: -0.7,
            z: 1.2,
        };
        let before = orient_projected(point, Rotation::new(camera));
        let horizontal_degrees = 13.0;
        camera.orbit(horizontal_degrees, 0.0);
        let after = orient_projected(point, Rotation::new(camera));
        let radians = -horizontal_degrees.to_radians();
        let expected = (
            before[0] * radians.cos() - before[1] * radians.sin(),
            before[0] * radians.sin() + before[1] * radians.cos(),
            before[2],
        );
        assert!((after[0] - expected.0).abs() < 0.00001);
        assert!((after[1] - expected.1).abs() < 0.00001);
        assert!((after[2] - expected.2).abs() < 0.00001);

        let before = after;
        let vertical_degrees = 9.0;
        camera.orbit(0.0, vertical_degrees);
        let after = orient_projected(point, Rotation::new(camera));
        let radians = vertical_degrees.to_radians();
        let expected = (
            before[0],
            before[1] * radians.cos() - before[2] * radians.sin(),
            before[1] * radians.sin() + before[2] * radians.cos(),
        );
        assert!((after[0] - expected.0).abs() < 0.00001);
        assert!((after[1] - expected.1).abs() < 0.00001);
        assert!((after[2] - expected.2).abs() < 0.00001);
    }

    #[test]
    fn incremental_raster_matches_reference_pixel_coverage_and_depth() {
        let triangles = [
            ([8.0, 9.0, 0.2], [91.0, 16.0, 0.8], [30.0, 88.0, 0.4]),
            ([10.25, 70.5, 0.7], [80.0, 61.0, 0.1], [74.0, 11.0, 0.6]),
            ([-4.0, 18.0, 0.3], [45.0, -7.0, 0.6], [98.0, 80.0, 0.5]),
        ];
        let color = [66, 132, 174, 255];
        let mut expected_rgba = vec![0; 96 * 96 * 4];
        let mut expected_depth = vec![f32::NEG_INFINITY; 96 * 96];
        let mut actual_rgba = expected_rgba.clone();
        let mut actual_depth = expected_depth.clone();
        let token = CancellationToken::new();

        for (a, b, c) in triangles {
            fill_triangle_reference(
                a,
                b,
                c,
                color,
                [96, 96],
                (&mut expected_rgba, &mut expected_depth),
            );
            fill_triangle(
                a,
                b,
                c,
                (color, MeshSurface::Solid),
                [96, 96],
                (&mut actual_rgba, &mut actual_depth),
                &token,
            )
            .unwrap();
        }

        assert_eq!(actual_rgba, expected_rgba);
        for (actual, expected) in actual_depth.iter().zip(expected_depth) {
            assert!(actual == &expected || (actual - expected).abs() < 0.00001);
        }
    }

    fn fill_triangle_reference(
        a: [f32; 3],
        b: [f32; 3],
        c: [f32; 3],
        color: [u8; 4],
        size: [u32; 2],
        buffers: (&mut [u8], &mut [f32]),
    ) {
        let (rgba, depth) = buffers;
        let [width, height] = size;
        let area = edge(a, b, c[0], c[1]);
        if area.abs() < 0.0001 {
            return;
        }
        let min_x = a[0].min(b[0]).min(c[0]).floor().max(0.0) as u32;
        let max_x = a[0].max(b[0]).max(c[0]).ceil().min(width as f32 - 1.0) as u32;
        let min_y = a[1].min(b[1]).min(c[1]).floor().max(0.0) as u32;
        let max_y = a[1].max(b[1]).max(c[1]).ceil().min(height as f32 - 1.0) as u32;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let w0 = edge(b, c, x as f32 + 0.5, y as f32 + 0.5) / area;
                let w1 = edge(c, a, x as f32 + 0.5, y as f32 + 0.5) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 >= -0.0001 && w1 >= -0.0001 && w2 >= -0.0001 {
                    let z = w0 * a[2] + w1 * b[2] + w2 * c[2];
                    let index = y as usize * width as usize + x as usize;
                    if z > depth[index] {
                        depth[index] = z;
                        rgba[index * 4..index * 4 + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }
}
