// Naygo — frames de imagen bajo demanda; protocolo acotado sin dependencias gráficas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use crate::preview_wire::{self, Response};
use std::io::{self, Read, Write};

pub const SIDE: u32 = 768;
pub const INTERVAL_US: u64 = 66_667;

#[derive(Debug, PartialEq)]
pub enum Event {
    Static(Response),
    Frame { at_us: u64, image: Response },
    End { at_us: u64 },
}

pub fn write(writer: &mut impl Write, event: &Event) -> io::Result<()> {
    let (kind, at) = match event {
        Event::Static(_) => (0, 0),
        Event::Frame { at_us, .. } => (1, *at_us),
        Event::End { at_us } => (2, *at_us),
    };
    writer.write_all(b"NGA1")?;
    writer.write_all(&[kind])?;
    writer.write_all(&at.to_le_bytes())?;
    match event {
        Event::Static(response) => preview_wire::write_response(writer, response)?,
        Event::Frame { image, .. } => {
            validate_frame(image)?;
            preview_wire::write_response(writer, image)?;
        }
        Event::End { .. } => {}
    }
    writer.flush()
}

fn validate_frame(response: &Response) -> io::Result<()> {
    match response {
        Response::Image { width, height, .. } if *width <= SIDE && *height <= SIDE => Ok(()),
        _ => Err(io::Error::other("invalid animation frame")),
    }
}

pub fn read(reader: &mut impl Read) -> io::Result<Event> {
    let mut header = [0; 13];
    reader.read_exact(&mut header)?;
    if &header[..4] != b"NGA1" {
        return Err(io::Error::other("invalid animation protocol"));
    }
    let at_us = u64::from_le_bytes(header[5..].try_into().unwrap());
    match header[4] {
        0 if at_us == 0 => Ok(Event::Static(preview_wire::read_response(reader)?)),
        1 => {
            // Animated frame dimensions are checked BEFORE allocating its body (≤768²×4).
            let image = preview_wire::read_response_with_side(reader, SIDE)?;
            validate_frame(&image)?;
            Ok(Event::Frame { at_us, image })
        }
        2 => Ok(Event::End { at_us }),
        _ => Err(io::Error::other("invalid animation event")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_animation_protocol_round_trip_and_truncation() {
        for event in [
            Event::End { at_us: 5_000_000 },
            Event::Frame {
                at_us: 0,
                image: Response::Image {
                    width: 1,
                    height: 1,
                    rgba: vec![0; 4],
                },
            },
            Event::Static(Response::Message("error".into())),
        ] {
            let mut bytes = Vec::new();
            write(&mut bytes, &event).unwrap();
            assert_eq!(read(&mut bytes.as_slice()).unwrap(), event);
            for end in 0..bytes.len() {
                assert!(read(&mut &bytes[..end]).is_err());
            }
        }
        assert!(read(&mut b"NGA1\xff\0\0\0\0\0\0\0\0".as_slice()).is_err());
    }
}
