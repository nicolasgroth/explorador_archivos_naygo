// Naygo — protocolo acotado del lector aislado, independiente de UI y Windows.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use std::io::{self, Read, Write};

pub const REQUEST_MAX: usize = 256 * 1024;
pub const TEXT_MAX: usize = 1024 * 1024;
pub const SIDE_MAX: u32 = crate::preview::IMAGE_MAX_SIDE;
const MAGIC: &[u8; 4] = b"NGP1";
#[path = "preview_text_wire.rs"]
mod styled;

#[derive(Debug, PartialEq)]
pub enum Response {
    Image {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Text {
        text: String,
        truncated: bool,
        bytes_read: Option<u32>,
    },
    Message(String),
    Scene(crate::mesh_preview::MeshScene),
    StyledText {
        text: String,
        truncated: bool,
        bytes_read: Option<u32>,
        lines: Vec<crate::highlight::HlLine>,
    },
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid preview protocol")
}

fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_bytes(reader: &mut impl Read, length: usize) -> io::Result<Vec<u8>> {
    // All callers validate lengths BEFORE allocation; allocation failure remains a Result.
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| invalid())?;
    bytes.resize(length, 0);
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub fn read_request(reader: &mut impl Read) -> io::Result<Vec<u8>> {
    let length = read_u32(reader)? as usize;
    if length == 0 || length > REQUEST_MAX {
        return Err(invalid());
    }
    read_bytes(reader, length)
}

pub fn write_request(writer: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() || bytes.len() > REQUEST_MAX {
        return Err(invalid());
    }
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(bytes)?;
    writer.flush()
}

fn image_len(width: u32, height: u32) -> io::Result<usize> {
    if width == 0 || height == 0 || width > SIDE_MAX || height > SIDE_MAX {
        return Err(invalid());
    }
    width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(4))
        .map(|n| n as usize)
        .ok_or_else(invalid)
}

pub fn write_response(writer: &mut impl Write, response: &Response) -> io::Result<()> {
    // Additional kinds fail closed in older readers. They share the same supervisor;
    // geometry is streamed directly, never serialized through an unbounded JSON tree.
    if let Response::Scene(scene) = response {
        writer.write_all(MAGIC)?;
        writer.write_all(&[5])?;
        for value in [0, 0, scene.wire_len()? as u32] {
            writer.write_all(&value.to_le_bytes())?;
        }
        scene.write_wire(writer)?;
        return writer.flush();
    }
    if let Response::StyledText {
        text,
        truncated,
        bytes_read,
        lines,
    } = response
    {
        writer.write_all(MAGIC)?;
        let kind = match (*truncated, bytes_read.is_some()) {
            (false, false) => 6,
            (true, false) => 7,
            (false, true) => 10,
            (true, true) => 11,
        };
        writer.write_all(&[kind])?;
        writer.write_all(&bytes_read.unwrap_or(0).to_le_bytes())?;
        writer.write_all(&[0; 8])?;
        styled::write(writer, text, lines)?;
        return writer.flush();
    }
    let (kind, width, height, data) = match response {
        Response::Image {
            width,
            height,
            rgba,
        } => {
            if image_len(*width, *height)? != rgba.len() {
                return Err(invalid());
            }
            (1u8, *width, *height, rgba.as_slice())
        }
        Response::Text {
            text,
            truncated,
            bytes_read,
        } => {
            let (kind, bytes_read) = match (*truncated, bytes_read) {
                (false, None) => (2, 0),
                (true, None) => (3, 0),
                (false, Some(bytes)) => (8, *bytes),
                (true, Some(bytes)) => (9, *bytes),
            };
            (kind, bytes_read, 0, text.as_bytes())
        }
        Response::Message(text) => (4, 0, 0, text.as_bytes()),
        Response::Scene(_) | Response::StyledText { .. } => unreachable!(),
    };
    if kind != 1 && data.len() > TEXT_MAX {
        return Err(invalid());
    }
    writer.write_all(MAGIC)?;
    writer.write_all(&[kind])?;
    for value in [width, height, data.len() as u32] {
        writer.write_all(&value.to_le_bytes())?;
    }
    writer.write_all(data)?;
    writer.flush()
}

pub fn read_response(reader: &mut impl Read) -> io::Result<Response> {
    read_response_with_side(reader, SIDE_MAX)
}

pub(crate) fn read_response_with_side(reader: &mut impl Read, side: u32) -> io::Result<Response> {
    let mut magic = [0; 4];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(invalid());
    }
    let mut kind = [0];
    reader.read_exact(&mut kind)?;
    let width = read_u32(reader)?;
    let height = read_u32(reader)?;
    let length = read_u32(reader)? as usize;
    match kind[0] {
        1 => {
            if width > side || height > side || image_len(width, height)? != length {
                return Err(invalid());
            }
            Ok(Response::Image {
                width,
                height,
                rgba: read_bytes(reader, length)?,
            })
        }
        2..=4 if width == 0 && height == 0 && length <= TEXT_MAX => {
            let text = String::from_utf8(read_bytes(reader, length)?).map_err(|_| invalid())?;
            Ok(if kind[0] == 4 {
                Response::Message(text)
            } else {
                Response::Text {
                    text,
                    truncated: kind[0] == 3,
                    bytes_read: None,
                }
            })
        }
        8..=9 if width as usize <= TEXT_MAX && height == 0 && length <= TEXT_MAX => {
            Ok(Response::Text {
                text: String::from_utf8(read_bytes(reader, length)?).map_err(|_| invalid())?,
                truncated: kind[0] == 9,
                bytes_read: Some(width),
            })
        }
        5 if side == SIDE_MAX
            && width == 0
            && height == 0
            && length <= crate::mesh_preview::SCENE_MAX_BYTES =>
        {
            Ok(Response::Scene(crate::mesh_preview::MeshScene::read_wire(
                reader, length,
            )?))
        }
        6..=7 if side == SIDE_MAX && width == 0 && height == 0 && length == 0 => {
            let (text, lines) = styled::read(reader)?;
            Ok(Response::StyledText {
                text,
                lines,
                truncated: kind[0] == 7,
                bytes_read: None,
            })
        }
        10..=11 if side == SIDE_MAX && width as usize <= TEXT_MAX && height == 0 && length == 0 => {
            let (text, lines) = styled::read(reader)?;
            Ok(Response::StyledText {
                text,
                lines,
                truncated: kind[0] == 11,
                bytes_read: Some(width),
            })
        }
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_all_messages_and_truncated_pipes() {
        for response in [
            Response::Image {
                width: 2,
                height: 1,
                rgba: vec![17; 8],
            },
            Response::Text {
                text: "á中".into(),
                truncated: true,
                bytes_read: Some(7),
            },
            Response::Message("error".into()),
        ] {
            let mut bytes = Vec::new();
            write_response(&mut bytes, &response).unwrap();
            assert_eq!(read_response(&mut bytes.as_slice()).unwrap(), response);
            for end in 0..bytes.len() {
                assert!(read_response(&mut &bytes[..end]).is_err());
            }
        }
    }
    #[test]
    fn rejects_lengths_dimensions_types_and_version_before_body() {
        for (kind, width, height, length) in [
            (1, u32::MAX, 1, 4),
            (1, 0, 1, 0),
            (1, 2, 2, 4),
            (4, 0, 0, u32::MAX),
            (2, 1, 0, 1),
            (99, 0, 0, 0),
        ] {
            let mut bytes = b"NGP1".to_vec();
            bytes.push(kind);
            for value in [width, height, length] {
                bytes.extend(value.to_le_bytes());
            }
            assert_eq!(
                read_response(&mut bytes.as_slice()).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        assert!(read_response(&mut b"NGP2".as_slice()).is_err());
        assert!(read_request(&mut u32::MAX.to_le_bytes().as_slice()).is_err());
        assert!(read_request(&mut 0u32.to_le_bytes().as_slice()).is_err());
    }
}
