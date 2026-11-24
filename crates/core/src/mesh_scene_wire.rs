// Naygo — transferencia binaria acotada de geometría, sin copias intermedias gigantes.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::{bounds, Mesh, MeshInfo, MeshScene, Triangle, Vec3};
use std::io::{self, Read, Write};

pub const SCENE_MAX_BYTES: usize = 48 * 1024 * 1024;
const UNIT_MAX: usize = 128;
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid preview geometry")
}
fn number(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
fn length(vertices: usize, triangles: usize, unit: usize, colors: bool) -> io::Result<usize> {
    if vertices == 0 || triangles == 0 || triangles > super::MESH_MAX_TRIANGLES || unit > UNIT_MAX {
        return Err(invalid());
    }
    vertices
        .checked_add(triangles)
        .and_then(|n| n.checked_mul(12))
        .and_then(|n| n.checked_add(12 + unit))
        .and_then(|n| {
            if colors {
                n.checked_add(triangles.checked_mul(5)?)
            } else {
                Some(n)
            }
        })
        .filter(|&n| n.checked_add(unit).is_some_and(|n| n <= SCENE_MAX_BYTES))
        .ok_or_else(invalid)
}
impl MeshScene {
    pub fn wire_len(&self) -> io::Result<usize> {
        if !self.mesh.colors.is_empty() && self.mesh.colors.len() != self.mesh.triangles.len() {
            return Err(invalid());
        }
        length(
            self.mesh.vertices.len(),
            self.mesh.triangles.len(),
            self.mesh.unit.len(),
            !self.mesh.colors.is_empty(),
        )
    }
    /// Streaming directo a la tubería. El supervisor impone tiempo/memoria al productor.
    pub fn write_wire(&self, writer: &mut impl Write) -> io::Result<()> {
        self.wire_len()?;
        for n in [
            self.mesh.vertices.len(),
            self.mesh.triangles.len()
                | if self.mesh.colors.is_empty() {
                    0
                } else {
                    1 << 31
                },
            self.mesh.unit.len(),
        ] {
            writer.write_all(&(n as u32).to_le_bytes())?;
        }
        for point in &self.mesh.vertices {
            for value in [point.x, point.y, point.z] {
                if !value.is_finite() {
                    return Err(invalid());
                }
                writer.write_all(&value.to_le_bytes())?;
            }
        }
        for triangle in &self.mesh.triangles {
            for index in triangle.0 {
                if index as usize >= self.mesh.vertices.len() {
                    return Err(invalid());
                }
                writer.write_all(&index.to_le_bytes())?;
            }
        }
        for color in &self.mesh.colors {
            match color {
                Some(rgba) => {
                    writer.write_all(&[1])?;
                    writer.write_all(rgba)?;
                }
                None => writer.write_all(&[0; 5])?,
            }
        }
        writer.write_all(self.mesh.unit.as_bytes())
    }
    /// Longitudes verificadas ANTES de reservar; una sola representación residente.
    /// Se ejecuta en el hilo de tubería, nunca en UI. EOF/cancelación liberan los parciales.
    pub fn read_wire(reader: &mut impl Read, bytes: usize) -> io::Result<Self> {
        let nv = number(reader)? as usize;
        let triangle_header = number(reader)?;
        let has_colors = triangle_header & (1 << 31) != 0;
        let nt = (triangle_header & !(1 << 31)) as usize;
        let nu = number(reader)? as usize;
        if length(nv, nt, nu, has_colors)? != bytes {
            return Err(invalid());
        }
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        vertices.try_reserve_exact(nv).map_err(|_| invalid())?;
        triangles.try_reserve_exact(nt).map_err(|_| invalid())?;
        for _ in 0..nv {
            let x = f32::from_bits(number(reader)?);
            let y = f32::from_bits(number(reader)?);
            let z = f32::from_bits(number(reader)?);
            if ![x, y, z].into_iter().all(f32::is_finite) {
                return Err(invalid());
            }
            vertices.push(Vec3 { x, y, z });
        }
        for _ in 0..nt {
            let indices = [number(reader)?, number(reader)?, number(reader)?];
            if indices.iter().any(|&i| i as usize >= nv) {
                return Err(invalid());
            }
            triangles.push(Triangle(indices));
        }
        let mut colors = Vec::new();
        if has_colors {
            colors.try_reserve_exact(nt).map_err(|_| invalid())?;
            for _ in 0..nt {
                let mut encoded = [0u8; 5];
                reader.read_exact(&mut encoded)?;
                colors.push(match encoded {
                    [0, 0, 0, 0, 0] => None,
                    [1, r, g, b, a] => Some([r, g, b, a]),
                    _ => return Err(invalid()),
                });
            }
        }
        let mut unit = vec![0; nu];
        reader.read_exact(&mut unit)?;
        let unit = String::from_utf8(unit).map_err(|_| invalid())?;
        let (min, max) = bounds(&vertices);
        let dimensions = [max.x - min.x, max.y - min.y, max.z - min.z];
        if !dimensions.into_iter().all(f32::is_finite) {
            return Err(invalid());
        }
        Ok(Self {
            info: MeshInfo {
                triangle_count: nt,
                dimensions,
                unit: unit.clone(),
            },
            mesh: Mesh {
                vertices,
                triangles,
                colors,
                unit,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body() -> Vec<u8> {
        let mut out = Vec::new();
        for n in [3u32, 1, 2] {
            out.extend(n.to_le_bytes());
        }
        for value in [0f32, 0., 0., 1., 0., 0., 0., 1., 0.] {
            out.extend(value.to_le_bytes());
        }
        for n in [0u32, 1, 2] {
            out.extend(n.to_le_bytes());
        }
        out.extend(b"mm");
        out
    }
    #[test]
    fn scene_roundtrip_and_every_truncation() {
        let bytes = body();
        let scene = MeshScene::read_wire(&mut bytes.as_slice(), bytes.len()).unwrap();
        assert_eq!(scene.info.dimensions, [1., 1., 0.]);
        assert!(scene.retained_bytes().unwrap() < SCENE_MAX_BYTES);
        let mut out = Vec::new();
        scene.write_wire(&mut out).unwrap();
        assert_eq!(out, bytes);
        for end in 0..bytes.len() {
            assert!(MeshScene::read_wire(&mut &bytes[..end], bytes.len()).is_err());
        }
    }
    #[test]
    fn scene_roundtrip_preserves_optional_material_color_and_rejects_bad_flag() {
        let bytes = body();
        let mut scene = MeshScene::read_wire(&mut bytes.as_slice(), bytes.len()).unwrap();
        scene.mesh.colors = vec![Some([12, 34, 56, 128])];
        let mut encoded = Vec::new();
        scene.write_wire(&mut encoded).unwrap();
        let decoded = MeshScene::read_wire(&mut encoded.as_slice(), encoded.len()).unwrap();
        assert_eq!(decoded.mesh.colors, scene.mesh.colors);
        assert_eq!(decoded.wire_len().unwrap(), encoded.len());
        let color_offset = 12 + 3 * 12 + 12;
        encoded[color_offset] = 2;
        assert!(MeshScene::read_wire(&mut encoded.as_slice(), encoded.len()).is_err());
        scene.mesh.colors.push(None);
        assert!(scene.wire_len().is_err());
    }
    #[test]
    fn scene_rejects_bad_counts_coordinates_and_indices() {
        for (offset, value) in [
            (0, u32::MAX),
            (4, 0),
            (8, 129),
            (12, f32::NAN.to_bits()),
            (48, 99),
        ] {
            let mut bytes = body();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(MeshScene::read_wire(&mut bytes.as_slice(), bytes.len()).is_err());
        }
        assert!(length(usize::MAX, 1, 0, false).is_err());
        assert!(length(SCENE_MAX_BYTES / 12, 1, 0, false).is_err());
        assert!(length(2_400_000, 1_500_000, 0, false).is_ok());
        assert!(length(2_400_000, 1_500_000, 0, true).is_err());
    }
}
