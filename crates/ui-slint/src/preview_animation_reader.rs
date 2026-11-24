// Naygo — composición secuencial WebP/GIF/APNG dentro del lector aislado.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use image::{AnimationDecoder, Frames, ImageDecoder, ImageFormat};
use naygo_core::preview_animation_wire::Event;
use naygo_core::preview_policy::Quality;
use naygo_core::preview_wire::Response;
use std::io::{self, Cursor};

fn invalid() -> io::Error {
    io::Error::other("invalid or oversized animation")
}
fn checked_decoder(decoder: &mut impl ImageDecoder) -> io::Result<()> {
    let (w, h) = decoder.dimensions();
    // Composition can hold several full canvases. This is a canvas bound, not a total
    // memory claim: the Windows Job additionally enforces 192 MiB committed per reader.
    if w == 0 || h == 0 || w > 8192 || h > 8192 || u64::from(w) * u64::from(h) > 4_000_000 {
        return Err(invalid());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    decoder.set_limits(limits).map_err(io::Error::other)
}

type Decoder<'a> = (Frames<'a>, Option<u32>);
fn open(bytes: &[u8]) -> io::Result<Option<Decoder<'_>>> {
    let cursor = Cursor::new(bytes);
    let result = match image::guess_format(bytes).map_err(io::Error::other)? {
        ImageFormat::WebP => {
            let mut decoder =
                image::codecs::webp::WebPDecoder::new(cursor).map_err(io::Error::other)?;
            if !decoder.has_animation() {
                return Ok(None);
            }
            checked_decoder(&mut decoder)?;
            let loops = loop_count(decoder.loop_count());
            (decoder.into_frames(), loops)
        }
        ImageFormat::Gif => {
            let (count, loops) = gif_info(bytes)?;
            if count <= 1 {
                return Ok(None);
            }
            let mut decoder =
                image::codecs::gif::GifDecoder::new(cursor).map_err(io::Error::other)?;
            checked_decoder(&mut decoder)?;
            (decoder.into_frames(), loops)
        }
        ImageFormat::Png => {
            let mut decoder =
                image::codecs::png::PngDecoder::new(cursor).map_err(io::Error::other)?;
            if !decoder.is_apng().map_err(io::Error::other)? {
                return Ok(None);
            }
            checked_decoder(&mut decoder)?;
            let decoder = decoder.apng().map_err(io::Error::other)?;
            let loops = loop_count(decoder.loop_count());
            (decoder.into_frames(), loops)
        }
        _ => return Ok(None),
    };
    Ok(Some(result))
}

fn loop_count(count: image::metadata::LoopCount) -> Option<u32> {
    match count {
        image::metadata::LoopCount::Infinite => None,
        image::metadata::LoopCount::Finite(n) => Some(n.get()),
    }
}

/// GIF's extension expresses repetitions AFTER the first pass; absence means one pass.
/// image's generic loop_count conflates absent/zero and may not yet have read the extension.
/// Scan only block boundaries, never search for magic inside compressed pixels. No allocations.
fn gif_info(bytes: &[u8]) -> io::Result<(u32, Option<u32>)> {
    fn take<'a>(bytes: &'a [u8], p: &mut usize, n: usize) -> io::Result<&'a [u8]> {
        let end = p.checked_add(n).ok_or_else(invalid)?;
        let value = bytes.get(*p..end).ok_or_else(invalid)?;
        *p = end;
        Ok(value)
    }
    let mut p = 0;
    let header = take(bytes, &mut p, 13)?;
    if header[10] & 0x80 != 0 {
        take(bytes, &mut p, 3 << ((header[10] & 7) + 1))?;
    }
    let mut count = 0u32;
    let mut loops = Some(1);
    loop {
        match take(bytes, &mut p, 1)?[0] {
            0x3b => return Ok((count, loops)),
            0x2c => {
                let descriptor = take(bytes, &mut p, 9)?;
                if descriptor[8] & 0x80 != 0 {
                    take(bytes, &mut p, 3 << ((descriptor[8] & 7) + 1))?;
                }
                take(bytes, &mut p, 1)?; // LZW minimum code size.
                loop {
                    let n = take(bytes, &mut p, 1)?[0] as usize;
                    if n == 0 {
                        break;
                    }
                    take(bytes, &mut p, n)?;
                }
                count = count.checked_add(1).ok_or_else(invalid)?;
            }
            0x21 => {
                let extension = take(bytes, &mut p, 1)?[0];
                let mut first = true;
                let mut netscape = false;
                loop {
                    let n = take(bytes, &mut p, 1)?[0] as usize;
                    if n == 0 {
                        break;
                    }
                    let block = take(bytes, &mut p, n)?;
                    if first {
                        netscape = extension == 0xff
                            && (block == b"NETSCAPE2.0" || block == b"ANIMEXTS1.0");
                    } else if netscape && block.len() == 3 && block[0] == 1 {
                        let repeat = u16::from_le_bytes([block[1], block[2]]);
                        loops = (repeat != 0).then_some(u32::from(repeat) + 1);
                    }
                    first = false;
                }
            }
            _ => return Err(invalid()),
        }
    }
}

pub(super) struct Reader<'a> {
    quality: Quality,
    bytes: &'a [u8],
    frames: Option<Frames<'a>>,
    loops: Option<u32>,
    passes: u32,
    pass_frames: u32,
    next_us: u64,
    last_sent: Option<u64>,
}
impl<'a> Reader<'a> {
    #[cfg(test)]
    pub(super) fn new(bytes: &'a [u8]) -> io::Result<Option<Self>> {
        Self::with_quality(bytes, Quality::default())
    }
    pub(super) fn with_quality(bytes: &'a [u8], quality: Quality) -> io::Result<Option<Self>> {
        Ok(open(bytes)?.map(|(frames, loops)| Self {
            quality,
            bytes,
            frames: Some(frames),
            loops,
            passes: 0,
            pass_frames: 0,
            next_us: 0,
            last_sent: None,
        }))
    }
    /// image's pinned decoders compose alpha, subframes and disposal at original size.
    /// Discard only PRESENTATIONS, never skip dependent decoder frames. No collect_frames.
    pub(super) fn next(&mut self) -> io::Result<Event> {
        loop {
            let next = self.frames.as_mut().ok_or_else(invalid)?.next();
            let frame = match next {
                Some(frame) => frame.map_err(io::Error::other)?,
                None => {
                    self.passes = self.passes.checked_add(1).ok_or_else(invalid)?;
                    if self.pass_frames <= 1 || self.loops.is_some_and(|n| self.passes >= n) {
                        return Ok(Event::End {
                            at_us: self.next_us,
                        });
                    }
                    // Drop the old canvas BEFORE allocating the next loop's decoder.
                    self.frames = None;
                    self.frames = Some(open(self.bytes)?.ok_or_else(invalid)?.0);
                    self.pass_frames = 0;
                    continue;
                }
            };
            self.pass_frames = self.pass_frames.checked_add(1).ok_or_else(invalid)?;
            let at_us = self.next_us;
            let (n, d) = frame.delay().numer_denom_ms();
            // Zero/tiny delays cannot turn malformed files into an unbounded hot loop.
            let delay = (u64::from(n) * 1000 / u64::from(d.max(1))).max(10_000);
            self.next_us = self.next_us.checked_add(delay).ok_or_else(invalid)?;
            if self
                .last_sent
                .is_some_and(|last| at_us.saturating_sub(last) < self.quality.interval_us())
            {
                continue;
            }
            self.last_sent = Some(at_us);
            let buffer = frame.into_buffer();
            let image = image::DynamicImage::ImageRgba8(buffer);
            let side = self.quality.animation_side();
            let image = if image.width() > side || image.height() > side {
                image.thumbnail(side, side)
            } else {
                image
            };
            let image = image.into_rgba8();
            return Ok(Event::Frame {
                at_us,
                image: Response::Image {
                    width: image.width(),
                    height: image.height(),
                    rgba: image.into_raw(),
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    mod fixtures {
        include!("../tests/fixtures/animated_images.rs");
    }
    #[test]
    fn gif_and_apng_partial_frames_restore_previous_canvas() {
        for bytes in [fixtures::apng(), fixtures::gif()] {
            let mut reader = Reader::new(&bytes).unwrap().unwrap();
            for (index, pixels) in [
                [255, 0, 0, 255, 255, 0, 0, 255],
                [255, 0, 0, 255, 0, 0, 255, 255],
                [0, 255, 0, 255, 255, 0, 0, 255],
                [0, 0, 0, 0, 255, 0, 0, 255],
            ]
            .iter()
            .enumerate()
            {
                assert!(
                    matches!(reader.next().unwrap(), Event::Frame { at_us, image: Response::Image { rgba, .. } }
                    if at_us == index as u64 * 100_000 && rgba == *pixels)
                );
            }
            assert_eq!(reader.next().unwrap(), Event::End { at_us: 400_000 });
        }
    }
    #[test]
    fn webp_alpha_finite_and_infinite_loops() {
        for loops in [0, 1, 2] {
            let bytes = fixtures::webp(loops);
            let mut reader = Reader::new(&bytes).unwrap().unwrap();
            for index in 0..if loops == 0 { 12 } else { u32::from(loops) * 2 } {
                let pixels = if index % 2 == 0 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 128]
                };
                assert!(
                    matches!(reader.next().unwrap(), Event::Frame { at_us, image: Response::Image { rgba, .. } }
                    if at_us == u64::from(index) * 100_000 && rgba == pixels.repeat(2))
                );
            }
            if loops > 0 {
                assert_eq!(
                    reader.next().unwrap(),
                    Event::End {
                        at_us: u64::from(loops) * 200_000
                    }
                );
            }
        }
    }
    #[test]
    fn excessive_canvas_and_truncated_animation_fail_bounded() {
        let mut bytes = fixtures::webp(1);
        bytes[24..27].copy_from_slice(&[0xff, 0x0f, 0]);
        bytes[27..30].copy_from_slice(&[0xff, 0x0f, 0]);
        assert!(Reader::new(&bytes).is_err());
        let bytes = fixtures::webp(1);
        for n in [0, 10, 30, bytes.len() - 10] {
            if let Ok(Some(mut reader)) = Reader::new(&bytes[..n]) {
                assert!((0..4).any(|_| reader.next().is_err()));
            }
        }
    }
    fn gif(repeat: Option<image::codecs::gif::Repeat>) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            if let Some(repeat) = repeat {
                encoder.set_repeat(repeat).unwrap();
            }
            for value in [0, 255] {
                encoder
                    .encode_frame(image::Frame::from_parts(
                        image::RgbaImage::from_pixel(2, 1, image::Rgba([value, 0, 0, 255])),
                        0,
                        0,
                        image::Delay::from_numer_denom_ms(100, 1),
                    ))
                    .unwrap();
            }
        }
        bytes
    }
    #[test]
    fn finite_gif_loops_and_timing_are_sequential_and_small_images_stay_small() {
        let bytes = gif(Some(image::codecs::gif::Repeat::Finite(1)));
        assert_eq!(gif_info(&bytes).unwrap(), (2, Some(2)));
        let mut reader = Reader::new(&bytes).unwrap().unwrap();
        for at in [0, 100_000, 200_000, 300_000] {
            assert!(
                matches!(reader.next().unwrap(), Event::Frame { at_us, image: Response::Image {width: 2, height: 1, ..} } if at_us == at)
            );
        }
        assert_eq!(reader.next().unwrap(), Event::End { at_us: 400_000 });
    }
    #[test]
    fn gif_without_loop_extension_ends_and_corrupt_block_fails() {
        let bytes = gif(None);
        assert_eq!(gif_info(&bytes).unwrap(), (2, Some(1)));
        for n in 0..bytes.len() {
            assert!(gif_info(&bytes[..n]).is_err());
        }
    }

    #[test]
    fn economic_animation_resizes_and_drops_presentations_not_composition() {
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            for value in [0, 40, 80, 120, 160] {
                encoder
                    .encode_frame(image::Frame::from_parts(
                        image::RgbaImage::from_pixel(800, 400, image::Rgba([value, 0, 0, 255])),
                        0,
                        0,
                        image::Delay::from_numer_denom_ms(40, 1),
                    ))
                    .unwrap();
            }
        }
        for (quality, side, times) in [
            (Quality::default(), 768, vec![0, 80_000, 160_000]),
            (Quality { economical: true }, 512, vec![0, 120_000]),
        ] {
            let mut reader = Reader::with_quality(&bytes, quality).unwrap().unwrap();
            for at in times {
                let Event::Frame {
                    at_us,
                    image:
                        Response::Image {
                            width,
                            height,
                            rgba,
                        },
                } = reader.next().unwrap()
                else {
                    panic!("frame expected")
                };
                assert_eq!((at_us, width, height), (at, side, side / 2));
                assert_eq!(rgba[0], (at / 1000) as u8);
            }
            assert_eq!(reader.next().unwrap(), Event::End { at_us: 200_000 });
        }
    }
}
