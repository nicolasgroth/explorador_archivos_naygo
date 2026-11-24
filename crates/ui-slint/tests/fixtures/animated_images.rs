// Naygo — fixtures sintéticas de animación; nunca contienen archivos del usuario.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

pub fn apng() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(4, 1).unwrap();
        encoder.set_frame_delay(1, 10).unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[255, 0, 0, 255, 255, 0, 0, 255])
            .unwrap();
        writer.set_frame_dimension(1, 1).unwrap();
        writer.set_frame_position(1, 0).unwrap();
        writer.set_blend_op(png::BlendOp::Over).unwrap();
        writer.set_dispose_op(png::DisposeOp::Previous).unwrap();
        writer.write_image_data(&[0, 0, 255, 255]).unwrap();
        writer.set_frame_position(0, 0).unwrap();
        writer.set_dispose_op(png::DisposeOp::Background).unwrap();
        writer.write_image_data(&[0, 255, 0, 255]).unwrap();
        writer.set_frame_position(1, 0).unwrap();
        writer.set_dispose_op(png::DisposeOp::None).unwrap();
        writer.write_image_data(&[0, 0, 0, 0]).unwrap();
        writer.finish().unwrap();
    }
    bytes
}

pub fn gif() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let palette = [255, 0, 0, 0, 0, 255, 0, 255, 0, 0, 0, 0];
        let mut encoder = gif::Encoder::new(&mut bytes, 2, 1, &palette).unwrap();
        let frame = gif::Frame {
            width: 2,
            height: 1,
            delay: 10,
            dispose: gif::DisposalMethod::Keep,
            buffer: std::borrow::Cow::Borrowed(&[0, 0]),
            ..Default::default()
        };
        encoder.write_frame(&frame).unwrap();
        let frame = gif::Frame {
            width: 1,
            height: 1,
            left: 1,
            delay: 10,
            dispose: gif::DisposalMethod::Previous,
            buffer: std::borrow::Cow::Borrowed(&[1]),
            ..Default::default()
        };
        encoder.write_frame(&frame).unwrap();
        let frame = gif::Frame {
            width: 1,
            height: 1,
            delay: 10,
            dispose: gif::DisposalMethod::Background,
            buffer: std::borrow::Cow::Borrowed(&[2]),
            ..Default::default()
        };
        encoder.write_frame(&frame).unwrap();
        let frame = gif::Frame {
            width: 1,
            height: 1,
            left: 1,
            delay: 10,
            transparent: Some(3),
            dispose: gif::DisposalMethod::Keep,
            buffer: std::borrow::Cow::Borrowed(&[3]),
            ..Default::default()
        };
        encoder.write_frame(&frame).unwrap();
    }
    bytes
}

pub fn webp(loops: u16) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        out.extend(tag);
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(data);
        if !data.len().is_multiple_of(2) {
            out.push(0);
        }
    }
    let mut body = b"WEBP".to_vec();
    chunk(&mut body, b"VP8X", &[0x12, 0, 0, 0, 1, 0, 0, 0, 0, 0]); // 2×1, alpha + animation.
    let mut anim = vec![0; 4];
    anim.extend(loops.to_le_bytes());
    chunk(&mut body, b"ANIM", &anim);
    for color in [[255, 0, 0, 255], [0, 0, 255, 128]] {
        let mut still = Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut still)
            .encode(&color.repeat(2), 2, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        let mut frame = vec![0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 100, 0, 0, 2];
        frame.extend(&still[12..]);
        chunk(&mut body, b"ANMF", &frame);
    }
    let mut result = b"RIFF".to_vec();
    result.extend((body.len() as u32).to_le_bytes());
    result.extend(body);
    result
}
